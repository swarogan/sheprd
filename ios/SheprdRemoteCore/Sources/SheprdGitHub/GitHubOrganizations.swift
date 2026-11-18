import Foundation

/// An organization the signed-in account belongs to, and what Sheprd's
/// GitHub App may read there.
public struct GitHubOrganization: Hashable, Sendable, Identifiable {
    public var login: String
    public var access: Access

    public init(login: String, access: Access) {
        self.login = login
        self.access = access
    }

    public var id: String { login.lowercased() }

    public enum Access: Hashable, Sendable {
        /// Installed on every repository.
        case all
        /// Installed on the repositories an owner chose.
        case selected
        /// Not installed: only public repositories are read. The screen
        /// links to `GitHubDeviceFlow.installURL`.
        case publicOnly
    }

    /// Organizations listed at most.
    static let limit = 100

    /// The account's organizations with the App's access to each, as the
    /// desktop's `installation.rs` checks it: the App's installations the
    /// account can see (`/user/installations`), against its organizations
    /// (`/user/orgs`). Only for an App user token (`ghu_`); nil when the
    /// installations cannot be read. Organizations that cannot be listed
    /// leave only those with an installation.
    public static func read(token: GitHubToken, transport: GitHubTransport) async -> [GitHubOrganization]? {
        guard token.secret.hasPrefix("ghu_"),
              let installations: Installations = await get("/user/installations?per_page=100", token: token, transport: transport)
        else { return nil }
        let orgs: [Account] = await get("/user/orgs?per_page=100", token: token, transport: transport) ?? []
        return merge(installations: installations.installations ?? [], orgs: orgs)
    }

    static func merge(installations: [Installation], orgs: [Account]) -> [GitHubOrganization] {
        var found: [String: GitHubOrganization] = [:]
        for org in orgs {
            guard let login = PullRequestSearchAnswer.name(org.login) else { continue }
            found[login.lowercased()] = GitHubOrganization(login: login, access: .publicOnly)
        }
        for installation in installations {
            guard installation.account?.type == "Organization", let login = PullRequestSearchAnswer.name(installation.account?.login) else { continue }
            let access: Access = installation.repositorySelection == "all" ? .all : .selected
            found[login.lowercased()] = GitHubOrganization(login: login, access: access)
        }
        return found.values
            .sorted { $0.login.localizedCaseInsensitiveCompare($1.login) == .orderedAscending }
            .prefix(limit).map { $0 }
    }

    struct Account: Decodable {
        var login: String?
        var type: String?
    }

    struct Installation: Decodable {
        var account: Account?
        var repositorySelection: String?

        enum CodingKeys: String, CodingKey {
            case account
            case repositorySelection = "repository_selection"
        }
    }

    struct Installations: Decodable {
        var installations: [Installation]?
    }

    private static func get<Value: Decodable>(_ path: String, token: GitHubToken, transport: GitHubTransport) async -> Value? {
        guard let url = URL(string: "https://api.github.com" + path),
              let response = try? await GitHubDeviceFlow.send(GitHubHTTP.request(url, token: token, timeout: 15), limit: GitHubAPI.restLimit, transport: transport),
              response.status == 200
        else { return nil }
        return try? JSONDecoder().decode(Value.self, from: response.body)
    }
}
