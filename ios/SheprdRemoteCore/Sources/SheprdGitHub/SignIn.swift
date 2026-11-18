import Foundation

/// Where the phone's own github.com sign-in stands, for Settings.
public enum GitHubSignInState: Equatable, Sendable {
    case idle
    /// Asking GitHub for a code.
    case requesting
    /// Waiting for the person to enter `code.userCode` at
    /// `code.verificationURI` before `code.expires`.
    case waiting(GitHubDeviceCode)
    case signedIn(login: String, access: GitHubAppAccess)
    case failed(GitHubSignInError)
}

/// The phone's own sign-in to github.com through the device flow, and the
/// token reads use with it. It renews the pair shortly before expiry or
/// when GitHub refuses the access token, one renewal at a time, and keeps
/// the rotated pair in the Keychain. GitHub invalidates the old pair as it
/// answers, as `sheprd-github`'s `token.rs` notes, so a rotated pair the
/// Keychain refuses is still the one used, held in memory and saved again
/// on the next read or renewal. A refresh token GitHub no longer takes
/// signs out.
public actor GitHubSignIn {
    private let flow: GitHubDeviceFlow
    private let store: GitHubSignInStore
    private var loaded = false
    private var loadFailure: GitHubSignInError?
    private var held: GitHubSignInCredential?
    /// `held` is a rotated pair the Keychain has not taken yet.
    private var unsaved = false
    private var access = GitHubAppAccess.unknown
    private var current = GitHubSignInState.idle
    /// Bumped by each start, cancel, and sign-out, so work they superseded
    /// changes nothing when it finishes.
    private var generation = 0
    private var polling: Task<Void, Never>?
    private var renewal: (id: Int, task: Task<GitHubSignInCredential, Error>)?
    private var renewals = 0
    private var watchers: [Int: AsyncStream<GitHubSignInState>.Continuation] = [:]
    private var watchCount = 0

    init(flow: GitHubDeviceFlow, store: GitHubSignInStore) {
        self.flow = flow
        self.store = store
    }

    /// The app's: Sheprd's GitHub App, the Keychain, GitHub over the
    /// network.
    public static func live() -> GitHubSignIn {
        GitHubSignIn(
            flow: GitHubDeviceFlow(transport: GitHubHTTP.urlSession()),
            store: GitHubSignInStore(vault: Keychain.vault(service: GitHubSignInStore.service))
        )
    }

    public var state: GitHubSignInState {
        loadOnce()
        return current
    }

    /// When the access token held expires; nil when signed out or for one
    /// that does not. It renews shortly before.
    public var expiry: Date? {
        loadOnce()
        return held?.expiresAt
    }

    /// The current state, then each change, until the stream is dropped.
    public func states() -> AsyncStream<GitHubSignInState> {
        loadOnce()
        let (stream, continuation) = AsyncStream.makeStream(of: GitHubSignInState.self, bufferingPolicy: .bufferingNewest(1))
        watchCount += 1
        let id = watchCount
        watchers[id] = continuation
        continuation.onTermination = { [weak self] _ in
            Task { await self?.unwatch(id) }
        }
        continuation.yield(current)
        return stream
    }

    /// Asks GitHub for a code and starts polling for it; answers the code to
    /// show. A sign-in already held stays in use until the new one
    /// completes.
    public func start() async throws -> GitHubDeviceCode {
        loadOnce()
        polling?.cancel()
        polling = nil
        generation += 1
        let mine = generation
        set(.requesting)
        let code: GitHubDeviceCode
        do {
            code = try await flow.requestCode()
        } catch {
            if mine == generation { set(Self.failure(error).map(GitHubSignInState.failed) ?? resting) }
            throw error
        }
        guard mine == generation else { throw CancellationError() }
        set(.waiting(code))
        polling = Task { await self.complete(code, generation: mine) }
        return code
    }

    /// Stops waiting for a code; a sign-in already held stays.
    public func cancel() {
        polling?.cancel()
        polling = nil
        generation += 1
        set(resting)
    }

    /// Deletes both tokens. Revoking the authorization itself is on GitHub
    /// (`GitHubDeviceFlow.applicationsURL`).
    public func signOut() throws {
        loadOnce()
        polling?.cancel()
        polling = nil
        generation += 1
        renewal = nil
        held = nil
        unsaved = false
        access = .unknown
        do {
            try store.delete()
        } catch {
            let failure = Self.failure(error) ?? .unreachable
            set(.failed(failure))
            throw failure
        }
        set(.idle)
    }

    /// The access token for a read, renewed first when it nears its
    /// expiry. When renewing fails before GitHub rotated the pair, for a
    /// reason other than GitHub refusing the refresh token, a token not yet
    /// expired is still used.
    public func token() async throws -> GitHubToken {
        let credential = try credential()
        saveUnsaved()
        let now = flow.now()
        guard credential.renewalDue(at: now) else { return credential.accessToken }
        do {
            return try await renew(credential).accessToken
        } catch let error as GitHubSignInError where error != .refreshRejected {
            guard let expires = credential.expiresAt, expires > flow.now() else { throw error }
            return credential.accessToken
        }
    }

    /// GitHub refused `token` (401): answers a renewed one, or the current
    /// one when another read already renewed it.
    func renew(rejected token: GitHubToken) async throws -> GitHubToken {
        let credential = try credential()
        guard credential.accessToken == token else { return credential.accessToken }
        guard credential.refreshToken != nil else { throw GitHubSignInError.unauthorized }
        return try await renew(credential).accessToken
    }

    private func credential() throws -> GitHubSignInCredential {
        loadOnce()
        if let held { return held }
        throw loadFailure ?? GitHubSignInError.signedOut
    }

    /// One renewal at a time: a second would spend the refresh token the
    /// first already rotated, and GitHub would sign this phone out. The
    /// renewal is its own task, so a read cancelled meanwhile cannot leave
    /// a rotated pair unsaved.
    private func renew(_ credential: GitHubSignInCredential) async throws -> GitHubSignInCredential {
        if let renewal { return try await renewal.task.value }
        renewals += 1
        let id = renewals
        let mine = generation
        let task = Task { try await self.performRenewal(of: credential, generation: mine) }
        renewal = (id, task)
        defer { if renewal?.id == id { renewal = nil } }
        return try await task.value
    }

    private func performRenewal(of credential: GitHubSignInCredential, generation mine: Int) async throws -> GitHubSignInCredential {
        let renewed: GitHubSignInCredential
        do {
            renewed = try await flow.refresh(credential)
        } catch GitHubSignInError.refreshRejected {
            if mine == generation {
                held = nil
                unsaved = false
                access = .unknown
                try? store.delete()
                set(.failed(.refreshRejected))
            }
            throw GitHubSignInError.refreshRejected
        }
        // Signed out, or signed in anew, while renewing: this pair is not
        // the one held any more.
        guard mine == generation else { throw GitHubSignInError.signedOut }
        // The old pair is gone on GitHub now: the new one is used even when
        // the Keychain refuses it, and saved again later.
        held = renewed
        unsaved = true
        saveUnsaved()
        return renewed
    }

    /// Saves a rotated pair the Keychain refused before; it stays held,
    /// for the next try, when the Keychain refuses it again.
    private func saveUnsaved() {
        guard unsaved, let held else { return }
        do {
            try store.save(held)
            unsaved = false
        } catch {
            // Kept in memory: this phone stays signed in until it quits.
        }
    }

    /// Polls for `code`, reads the account, and keeps the sign-in.
    private func complete(_ code: GitHubDeviceCode, generation mine: Int) async {
        do {
            var credential = try await flow.poll(code)
            credential.login = try await GitHubAccount.login(token: credential.accessToken, transport: flow.transport, now: flow.now())
            guard mine == generation else { return }
            try store.save(credential)
            held = credential
            unsaved = false
            access = .unknown
            polling = nil
            set(.signedIn(login: credential.login, access: .unknown))
            let found = await GitHubAccount.access(token: credential.accessToken, transport: flow.transport)
            guard mine == generation, held?.accessToken == credential.accessToken else { return }
            access = found
            set(.signedIn(login: credential.login, access: found))
        } catch {
            guard mine == generation else { return }
            polling = nil
            set(Self.failure(error).map(GitHubSignInState.failed) ?? resting)
        }
    }

    /// The state when no sign-in is under way.
    private var resting: GitHubSignInState {
        held.map { .signedIn(login: $0.login, access: access) } ?? .idle
    }

    private func loadOnce() {
        guard !loaded else { return }
        loaded = true
        do {
            held = try store.load()
        } catch {
            loadFailure = Self.failure(error) ?? .unreachable
        }
        current = loadFailure.map(GitHubSignInState.failed) ?? resting
    }

    private func set(_ state: GitHubSignInState) {
        guard state != current else { return }
        current = state
        for watcher in watchers.values { watcher.yield(state) }
    }

    private func unwatch(_ id: Int) {
        watchers[id] = nil
    }

    /// The failure to show; nil for a cancellation, which shows none.
    private static func failure(_ error: Error) -> GitHubSignInError? {
        switch error {
        case is CancellationError: nil
        case let error as GitHubSignInError: error
        default: .unreachable
        }
    }
}
