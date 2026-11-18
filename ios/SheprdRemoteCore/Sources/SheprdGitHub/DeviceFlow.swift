import Foundation

/// Why signing in to github.com on this phone failed, or why its sign-in
/// stopped working.
public enum GitHubSignInError: Error, Equatable, Sendable {
    /// GitHub's device code reply failed a check; the field it failed.
    case invalidDeviceCode(DeviceCodeField)
    /// The code expired before it was entered (`expired_token`, or the
    /// deadline passed while polling).
    case expired
    /// The person declined on GitHub (`access_denied`).
    case denied
    /// Another refusal of the authorization: a disabled device flow, an
    /// unknown client, an enterprise policy.
    case refused
    /// GitHub answered without a usable access token.
    case noToken
    /// A token type other than `bearer`.
    case tokenType
    /// GitHub no longer takes the refresh token (`bad_refresh_token`): the
    /// authorization was revoked, or the pair rotated elsewhere. Signed out.
    case refreshRejected
    /// No sign-in is held.
    case signedOut
    /// GitHub refused the access token (401) and there is no refresh token
    /// to renew it with.
    case unauthorized
    /// The Keychain refused to keep or read the sign-in.
    case storage(OSStatus)
    /// Reading the account (`GET /user`) failed.
    case profile(GitHubError)
    /// An HTTP status the flow does not expect.
    case status(Int)
    /// An answer this phone cannot read.
    case malformed
    /// GitHub could not be reached.
    case unreachable
}

/// The checks a device code reply must pass, as `sheprd-github`'s
/// `device.rs` makes them.
public enum DeviceCodeField: String, Equatable, Sendable {
    case deviceCode = "device_code"
    case userCode = "user_code"
    case verificationURI = "verification_uri"
    case expiresIn = "expires_in"
    case interval
}

/// A device code GitHub handed out: what the person types at
/// `verificationURI`, and what the phone polls with. The device code itself
/// is a secret and is redacted like a token.
public struct GitHubDeviceCode: Equatable, Sendable {
    let deviceCode: GitHubToken
    /// Shown to the person, with a copy button.
    public let userCode: String
    public let verificationURI: URL
    /// When the code stops working.
    public let expires: Date
    /// Seconds between polls, as GitHub first asked.
    let interval: Int
}

/// The phone's own authorization for github.com: its token pair, separate
/// from any computer's, so renewing one never signs the other out.
struct GitHubSignInCredential: Codable, Equatable, Sendable {
    static let version = 1

    var version: Int
    var accessToken: GitHubToken
    var refreshToken: GitHubToken?
    var clientID: String
    /// When the access token expires; nil for one that does not.
    var expiresAt: Date?
    /// The account, from `GET /user`; empty until it was read.
    var login: String

    init(accessToken: GitHubToken, refreshToken: GitHubToken?, clientID: String, expiresAt: Date?, login: String = "") {
        version = Self.version
        self.accessToken = accessToken
        self.refreshToken = refreshToken
        self.clientID = clientID
        self.expiresAt = expiresAt
        self.login = login
    }

    /// The checks `token.rs` makes before keeping or using a pair.
    var valid: Bool {
        version == Self.version && GitHubDeviceFlow.validToken(accessToken.secret)
            && (refreshToken.map { GitHubDeviceFlow.validToken($0.secret) } ?? true)
            && GitHubDeviceFlow.validClientID(clientID)
    }

    /// How long before expiry the pair is renewed, as the desktop does.
    static let renewal: TimeInterval = 10 * 60

    /// Whether to renew before the next read at `now`.
    func renewalDue(at now: Date) -> Bool {
        refreshToken != nil && expiresAt.map { $0.timeIntervalSince(now) <= Self.renewal } == true
    }

    enum CodingKeys: String, CodingKey {
        case version, login
        case accessToken = "access_token"
        case refreshToken = "refresh_token"
        case clientID = "client_id"
        case expiresAt = "expires_at"
    }
}

/// GitHub's device authorization flow with Sheprd's GitHub App, which
/// needs no client secret: a code to show, polling until the person
/// enters it, and renewing the pair it yields. Transport, clock, and sleep
/// are injected, so tests run it from a script without waiting.
public struct GitHubDeviceFlow: Sendable {
    /// Sheprd's own GitHub App, the one the desktop signs in with.
    static let clientID = "Iv23liurUcwxPjrdIFYT"
    /// Where the person enters the code; any other address in a reply is
    /// refused.
    static let verificationURI = "https://github.com/login/device"
    /// Where an organization installs the App, without which only its
    /// public repositories can be read.
    public static let installURL = "https://github.com/apps/herdr-ui/installations/new"
    /// Where the person revokes the authorization itself.
    public static let applicationsURL = "https://github.com/settings/apps/authorizations"

    /// GitHub's OAuth answers are a few hundred bytes.
    static let replyLimit = 64 * 1024

    typealias Sleep = @Sendable (Duration) async throws -> Void

    let clientID: String
    let transport: GitHubTransport
    let now: @Sendable () -> Date
    let sleep: Sleep

    init(
        clientID: String = GitHubDeviceFlow.clientID,
        transport: @escaping GitHubTransport,
        now: @escaping @Sendable () -> Date = { Date() },
        sleep: @escaping Sleep = { try await Task.sleep(for: $0) }
    ) {
        self.clientID = clientID
        self.transport = transport
        self.now = now
        self.sleep = sleep
    }

    /// Asks GitHub for a code. GitHub Apps ignore `scope` and use their
    /// registered permissions; an OAuth App would read it.
    func requestCode() async throws -> GitHubDeviceCode {
        let started = now()
        let response = try await post("device/code", ["client_id": clientID, "scope": "repo"])
        guard response.status == 200 else { throw GitHubSignInError.status(response.status) }
        struct Reply: Decodable {
            var device_code: String
            var user_code: String
            var verification_uri: String
            var expires_in: Int
            var interval: Int?
        }
        guard let reply = try? JSONDecoder().decode(Reply.self, from: response.body) else { throw GitHubSignInError.malformed }
        let interval = reply.interval ?? 5
        let userCode = reply.user_code
        if !Self.validToken(reply.device_code) {
            throw GitHubSignInError.invalidDeviceCode(.deviceCode)
        }
        if userCode.isEmpty || userCode.utf8.count > 32 || !userCode.unicodeScalars.allSatisfy({ $0.isASCII && (Self.alphanumeric.contains($0) || $0 == "-") }) {
            throw GitHubSignInError.invalidDeviceCode(.userCode)
        }
        guard reply.verification_uri == Self.verificationURI, let uri = URL(string: reply.verification_uri) else {
            throw GitHubSignInError.invalidDeviceCode(.verificationURI)
        }
        guard (1...900).contains(reply.expires_in) else { throw GitHubSignInError.invalidDeviceCode(.expiresIn) }
        guard (1...900).contains(interval) else { throw GitHubSignInError.invalidDeviceCode(.interval) }
        return GitHubDeviceCode(
            deviceCode: GitHubToken(reply.device_code), userCode: userCode, verificationURI: uri,
            expires: started.addingTimeInterval(TimeInterval(reply.expires_in)), interval: interval
        )
    }

    /// Polls until the person enters `code`, waiting as GitHub asks: its
    /// interval between polls, five seconds more after each `slow_down`.
    /// Cancelling the task stops it.
    func poll(_ code: GitHubDeviceCode) async throws -> GitHubSignInCredential {
        var interval = code.interval
        while true {
            try await sleep(.seconds(interval))
            try Task.checkCancellation()
            guard now() < code.expires else { throw GitHubSignInError.expired }
            let response = try await post("oauth/access_token", [
                "client_id": clientID,
                "device_code": code.deviceCode.secret,
                "grant_type": "urn:ietf:params:oauth:grant-type:device_code",
            ])
            switch try reply(response) {
            case .pending: continue
            case .slowDown: interval += 5
            case let .token(credential):
                // A token that arrives after the code expired is not used.
                guard now() < code.expires else { throw GitHubSignInError.expired }
                return credential
            }
        }
    }

    /// Renews `credential` with its refresh token. GitHub rotates the pair:
    /// the old one stops working once this answers, so the caller keeps the
    /// new one before using it.
    func refresh(_ credential: GitHubSignInCredential) async throws -> GitHubSignInCredential {
        guard let refresh = credential.refreshToken else { throw GitHubSignInError.signedOut }
        let response = try await post("oauth/access_token", [
            "client_id": credential.clientID,
            "grant_type": "refresh_token",
            "refresh_token": refresh.secret,
        ])
        switch try reply(response, refreshing: true) {
        case var .token(renewed):
            renewed.login = credential.login
            return renewed
        case .pending, .slowDown: throw GitHubSignInError.malformed
        }
    }

    enum Reply {
        case pending
        case slowDown
        case token(GitHubSignInCredential)
    }

    /// What an `access_token` answer means. GitHub answers OAuth errors with
    /// 200 and an `error` field; its description is never kept, as it may
    /// echo what was sent.
    func reply(_ response: GitHubResponse, refreshing: Bool = false) throws -> Reply {
        struct Body: Decodable {
            var access_token: String?
            var refresh_token: String?
            var expires_in: Int?
            var token_type: String?
            var error: String?
        }
        guard let body = try? JSONDecoder().decode(Body.self, from: response.body) else {
            throw response.status == 200 ? GitHubSignInError.malformed : GitHubSignInError.status(response.status)
        }
        switch body.error {
        case "authorization_pending"?: return .pending
        case "slow_down"?: return .slowDown
        case "expired_token"?: throw GitHubSignInError.expired
        case "access_denied"?: throw GitHubSignInError.denied
        case "bad_refresh_token"? where refreshing: throw GitHubSignInError.refreshRejected
        case _?: throw GitHubSignInError.refused
        case nil: break
        }
        guard response.status == 200 else { throw GitHubSignInError.status(response.status) }
        guard let access = body.access_token, Self.validToken(access) else { throw GitHubSignInError.noToken }
        guard body.token_type?.lowercased() == "bearer" else { throw GitHubSignInError.tokenType }
        let credential = GitHubSignInCredential(
            accessToken: GitHubToken(access),
            refreshToken: body.refresh_token.map(GitHubToken.init),
            clientID: clientID,
            expiresAt: body.expires_in.flatMap { $0 > 0 ? now().addingTimeInterval(TimeInterval($0)) : nil }
        )
        guard credential.valid else { throw GitHubSignInError.noToken }
        return .token(credential)
    }

    /// Posts a form to `https://github.com/login/PATH`, asking for JSON.
    private func post(_ path: String, _ fields: KeyValuePairs<String, String>) async throws -> GitHubResponse {
        guard let url = URL(string: "https://github.com/login/\(path)") else { throw GitHubSignInError.malformed }
        var request = URLRequest(url: url, cachePolicy: .reloadIgnoringLocalCacheData, timeoutInterval: 15)
        request.httpMethod = "POST"
        request.setValue("Sheprd", forHTTPHeaderField: "User-Agent")
        request.setValue("application/json", forHTTPHeaderField: "Accept")
        request.setValue("application/x-www-form-urlencoded", forHTTPHeaderField: "Content-Type")
        request.httpBody = Data(Self.form(fields).utf8)
        return try await Self.send(request, limit: Self.replyLimit, transport: transport)
    }

    /// Sends through `transport`, reading any failure but cancellation as
    /// this flow's own.
    static func send(_ request: URLRequest, limit: Int, transport: GitHubTransport) async throws -> GitHubResponse {
        do {
            return try await transport(request, .whole(max: limit))
        } catch is CancellationError {
            throw CancellationError()
        } catch let error as URLError where error.code == .cancelled {
            throw CancellationError()
        } catch GitHubError.tooLarge {
            throw GitHubSignInError.malformed
        } catch {
            throw GitHubSignInError.unreachable
        }
    }

    static func form(_ fields: KeyValuePairs<String, String>) -> String {
        fields.map { "\(encode($0.key))=\(encode($0.value))" }.joined(separator: "&")
    }

    private static let unreserved = CharacterSet(charactersIn: "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~")
    private static let alphanumeric = CharacterSet(charactersIn: "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789")

    private static func encode(_ text: String) -> String {
        text.addingPercentEncoding(withAllowedCharacters: unreserved) ?? ""
    }

    /// `store.rs`'s `valid_token`: printable ASCII, at most 4096 bytes.
    static func validToken(_ token: String) -> Bool {
        !token.isEmpty && token.utf8.count <= 4096 && token.utf8.allSatisfy { (0x21...0x7E).contains($0) }
    }

    static func validClientID(_ id: String) -> Bool {
        !id.isEmpty && id.utf8.count <= 256
            && id.unicodeScalars.allSatisfy { $0.isASCII && (alphanumeric.contains($0) || "_-.".unicodeScalars.contains($0)) }
    }
}
