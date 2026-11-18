import Foundation
import Synchronization
import Testing
@testable import SheprdGitHub
@testable import SheprdRemoteCore

struct SignInTests {
    private let clock = TestClock()
    private let transport = FakeTransport()
    private let vault = SecretVault.memory()
    private var store: GitHubSignInStore { GitHubSignInStore(vault: vault) }

    private func signIn(transport: GitHubTransport? = nil, sleep: GitHubDeviceFlow.Sleep? = nil) -> GitHubSignIn {
        GitHubSignIn(
            flow: GitHubDeviceFlow(transport: transport ?? self.transport.transport, now: clock.read, sleep: sleep ?? TestSleeper(clock: clock).sleep),
            store: store
        )
    }

    /// Waits for `signIn` to reach a state other than `.waiting`.
    private func settled(_ signIn: GitHubSignIn) async -> GitHubSignInState {
        for await state in await signIn.states() {
            switch state {
            case .waiting, .requesting: continue
            case .signedIn(_, .unknown): continue
            default: return state
            }
        }
        return await signIn.state
    }

    @Test func signingInKeepsThePairAndTheLogin() async throws {
        transport.enqueue(SignInFixtures.code())
        transport.enqueue(SignInFixtures.error("authorization_pending"))
        transport.enqueue(SignInFixtures.token("ghu_a1"))
        transport.enqueue(SignInFixtures.user("ana"))
        transport.enqueue(SignInFixtures.installations(0))
        let signIn = signIn()
        #expect(await signIn.state == .idle)
        let code = try await signIn.start()
        #expect(code.userCode == "WDJB-MJHT")
        #expect(await settled(signIn) == .signedIn(login: "ana", access: .notInstalled))
        let kept = try #require(try store.load())
        #expect(kept.accessToken == GitHubToken("ghu_a1"))
        #expect(kept.refreshToken == GitHubToken("ghr_r1"))
        #expect(kept.login == "ana")
        #expect(transport.requests.map { $0.url?.path() } == ["/login/device/code", "/login/oauth/access_token", "/login/oauth/access_token", "/user", "/user/installations"])
        #expect(transport.requests[3].value(forHTTPHeaderField: "Authorization") == "Bearer ghu_a1")
        #expect(try await signIn.token() == GitHubToken("ghu_a1"))
    }

    @Test func aHeldSignInIsReadOnLaunch() async throws {
        try store.save(SignInFixtures.credential())
        #expect(await signIn().state == .signedIn(login: "ana", access: .unknown))
    }

    @Test func aDeniedCodeFails() async throws {
        transport.enqueue(SignInFixtures.code())
        transport.enqueue(SignInFixtures.error("access_denied"))
        let signIn = signIn()
        _ = try await signIn.start()
        #expect(await settled(signIn) == .failed(.denied))
        #expect(try store.load() == nil)
    }

    @Test func cancellingReturnsToTheSignInHeld() async throws {
        try store.save(SignInFixtures.credential())
        transport.enqueue(SignInFixtures.code())
        let signIn = signIn(sleep: { try await Task.sleep(for: .seconds($0.components.seconds * 1000)) })
        _ = try await signIn.start()
        guard case .waiting = await signIn.state else { Issue.record("not waiting"); return }
        await signIn.cancel()
        #expect(await signIn.state == .signedIn(login: "ana", access: .unknown))
        #expect(transport.requests.count == 1)
    }

    @Test func aPairNearItsExpiryIsRenewedAndKeptBeforeUse() async throws {
        try store.save(SignInFixtures.credential(expiresIn: 9 * 60))
        transport.enqueue(SignInFixtures.token("ghu_a2", refresh: "ghr_r2"))
        let token = try await signIn().token()
        #expect(token == GitHubToken("ghu_a2"))
        let kept = try #require(try store.load())
        #expect(kept.accessToken == GitHubToken("ghu_a2"))
        #expect(kept.refreshToken == GitHubToken("ghr_r2"))
        #expect(kept.login == "ana")
    }

    @Test func aRefusedTokenIsRenewedOnceForConcurrentReads() async throws {
        try store.save(SignInFixtures.credential())
        transport.enqueue(SignInFixtures.token("ghu_a2", refresh: "ghr_r2"))
        let signIn = signIn()
        async let first = signIn.renew(rejected: GitHubToken("ghu_a1"))
        async let second = signIn.renew(rejected: GitHubToken("ghu_a1"))
        #expect(try await [first, second] == [GitHubToken("ghu_a2"), GitHubToken("ghu_a2")])
        #expect(transport.requests.count == 1)
        // A read that saw the old token after the renewal gets the new one.
        #expect(try await signIn.renew(rejected: GitHubToken("ghu_a1")) == GitHubToken("ghu_a2"))
        #expect(transport.requests.count == 1)
    }

    @Test func aRevokedRefreshTokenSignsOut() async throws {
        try store.save(SignInFixtures.credential())
        transport.enqueue(SignInFixtures.error("bad_refresh_token"))
        let signIn = signIn()
        await #expect(throws: GitHubSignInError.refreshRejected) { try await signIn.renew(rejected: GitHubToken("ghu_a1")) }
        #expect(await signIn.state == .failed(.refreshRejected))
        #expect(try store.load() == nil)
        await #expect(throws: GitHubSignInError.signedOut) { try await signIn.token() }
    }

    @Test func aTokenWithoutRefreshIsNotRenewed() async throws {
        try store.save(SignInFixtures.credential(refresh: nil, expiresIn: nil))
        let signIn = signIn()
        #expect(try await signIn.token() == GitHubToken("ghu_a1"))
        await #expect(throws: GitHubSignInError.unauthorized) { try await signIn.renew(rejected: GitHubToken("ghu_a1")) }
        #expect(transport.requests.isEmpty)
    }

    @Test func signingOutDeletesBothTokens() async throws {
        try store.save(SignInFixtures.credential())
        let signIn = signIn()
        try await signIn.signOut()
        #expect(try vault.read(GitHubSignInStore.account) == nil)
        #expect(await signIn.state == .idle)
        await #expect(throws: GitHubSignInError.signedOut) { try await signIn.token() }
    }

    @Test func theOwnSignInAndLentTokensAreKeptApart() throws {
        #expect(GitHubSignInStore.service != TokenStore.service)
        try store.save(SignInFixtures.credential())
        try vault.write(Data("not a sign-in".utf8), GitHubSignInStore.account)
        // An item this version cannot read is dropped, not used.
        #expect(try store.load() == nil)
        #expect(try vault.read(GitHubSignInStore.account) == nil)
    }

    @Test func tokensAreRedacted() throws {
        let credential = SignInFixtures.credential()
        var dumped = ""
        dump(credential, to: &dumped)
        let code = GitHubDeviceCode(deviceCode: GitHubToken("dc-secret"), userCode: "AB-12", verificationURI: URL(string: "https://github.com/login/device")!, expires: GitHubFixtures.start, interval: 5)
        var dumpedCode = ""
        dump(code, to: &dumpedCode)
        for text in [String(describing: credential), String(reflecting: credential), dumped, String(describing: code), dumpedCode] {
            #expect(!text.contains("ghu_a1"))
            #expect(!text.contains("ghr_r1"))
            #expect(!text.contains("dc-secret"))
        }
        let encoded = String(decoding: try JSONEncoder().encode(credential), as: UTF8.self)
        #expect(encoded.contains("ghr_r1"))
    }
}
