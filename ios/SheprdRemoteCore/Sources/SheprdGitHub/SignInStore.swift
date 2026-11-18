import Foundation

/// Where the phone keeps its own github.com sign-in: the Keychain, with
/// the attributes `TokenStore` uses (this device only, never synchronized,
/// no shared access group), under a service of its own so a lent token and
/// the own sign-in never overwrite or enumerate each other.
struct GitHubSignInStore: Sendable {
    /// The Keychain service the app keeps it under.
    static let service = "so.pen.herdr.remote.github.signin"
    /// The one item's account: the App exists on github.com alone.
    static let account = "github.com"

    let vault: SecretVault

    init(vault: SecretVault) {
        self.vault = vault
    }

    /// The sign-in held; an item this version cannot read, or one that
    /// fails the pair's checks, is dropped, as signing in again replaces it.
    func load() throws -> GitHubSignInCredential? {
        let data: Data?
        do {
            data = try vault.read(Self.account)
        } catch let failure as Keychain.Failure {
            throw GitHubSignInError.storage(failure.status)
        }
        guard let data else { return nil }
        guard let credential = try? JSONDecoder().decode(GitHubSignInCredential.self, from: data), credential.valid else {
            try? vault.delete(Self.account)
            return nil
        }
        return credential
    }

    func save(_ credential: GitHubSignInCredential) throws {
        guard credential.valid else { throw GitHubSignInError.noToken }
        do {
            try vault.write(JSONEncoder().encode(credential), Self.account)
        } catch let failure as Keychain.Failure {
            throw GitHubSignInError.storage(failure.status)
        }
    }

    /// Deletes both tokens.
    func delete() throws {
        do {
            try vault.delete(Self.account)
        } catch let failure as Keychain.Failure {
            throw GitHubSignInError.storage(failure.status)
        }
    }
}

/// What Sheprd's GitHub App can read for the signed-in account.
public enum GitHubAppAccess: Equatable, Sendable {
    /// Not checked, or the check failed.
    case unknown
    /// Installed on at least one account or organization.
    case installed
    /// Installed nowhere: only public repositories are read. The screen
    /// links to `GitHubDeviceFlow.installURL`.
    case notInstalled
}

enum GitHubAccount {
    /// The login `token` belongs to (`GET /user`).
    static func login(token: GitHubToken, transport: GitHubTransport, now: Date) async throws -> String {
        let response = try await get("/user", token: token, transport: transport)
        guard response.status == 200 else { throw GitHubSignInError.profile(GitHubHTTP.failure(response, now: now)) }
        struct User: Decodable { var login: String }
        guard let user = try? JSONDecoder().decode(User.self, from: response.body), !user.login.isEmpty, user.login.utf8.count <= 256 else {
            throw GitHubSignInError.profile(.malformed)
        }
        return user.login
    }

    /// Whether the App is installed anywhere this account reaches, as the
    /// desktop's `installation.rs` checks: only for an App user token
    /// (`ghu_`), from the first page of `/user/installations`.
    static func access(token: GitHubToken, transport: GitHubTransport) async -> GitHubAppAccess {
        guard token.secret.hasPrefix("ghu_"),
              let response = try? await get("/user/installations?per_page=100", token: token, transport: transport),
              response.status == 200
        else { return .unknown }
        struct Listing: Decodable {
            struct Installation: Decodable { var id: UInt64 }
            var installations: [Installation]?
        }
        guard let listing = try? JSONDecoder().decode(Listing.self, from: response.body) else { return .unknown }
        return (listing.installations ?? []).isEmpty ? .notInstalled : .installed
    }

    private static func get(_ path: String, token: GitHubToken, transport: GitHubTransport) async throws -> GitHubResponse {
        guard let url = URL(string: "https://api.github.com" + path) else { throw GitHubSignInError.malformed }
        return try await GitHubDeviceFlow.send(GitHubHTTP.request(url, token: token, timeout: 15), limit: GitHubAPI.restLimit, transport: transport)
    }
}
