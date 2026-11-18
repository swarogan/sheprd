import Foundation
import SheprdRemoteCore

/// What reading the pull request lists came to.
public enum PullRequestListRead: Hashable, Sendable {
    case found(PullRequestLists)
    /// No github.com token: the phone is signed out and no paired computer
    /// lends an account. Settings > GitHub fixes that.
    case noAccount
    /// A token was there, but GitHub could not be read with it.
    case failed
}

extension PullRequestReader {
    /// The host the lists come from: Sheprd's GitHub App, and so the
    /// phone's own sign-in, exist on github.com alone.
    static let listHost = GitHubHost("github.com")!

    /// The signed-in account's open pull requests and those asking for its
    /// review, with the phone's own sign-in first, then a lent token. There
    /// is no computer to fall back to: a search spans repositories no
    /// computer has.
    public func pullRequestLists(source: Lenders) async -> PullRequestListRead {
        let host = Self.listHost
        guard await hasAccount(for: host, source: source) else { return .noAccount }
        let found = await attempt(host: host, source: source) { api, token in
            try await api.searchPullRequests(host: host, token: token)
        }
        return found.map(PullRequestListRead.found) ?? .failed
    }

    /// Whether any token for `host` could be had: the own sign-in, a lease
    /// held, or a computer that lends one.
    func hasAccount(for host: GitHubHost, source: Lenders) async -> Bool {
        if host == Self.listHost, let signIn, case .signedIn = await signIn.state { return true }
        if !GitHubLender.order(source.lenders, for: host).isEmpty { return true }
        return await tokens.all().contains { $0.host == host }
    }

    /// Forgets the answers read so far, as signing out does: they were
    /// read with the token that is gone.
    public func forgetAnswers() async {
        await api.forgetAnswers()
    }
}

extension GitHubAPI {
    func forgetAnswers() {
        etags = ETagCache()
    }
}

/// Where a pull request's branch is checked out: a workspace on one of the
/// paired computers.
public struct WorkspacePlace: Hashable, Sendable {
    public var device: String
    public var computer: String
    public var workspace: String

    public init(device: String, computer: String, workspace: String) {
        self.device = device
        self.computer = computer
        self.workspace = workspace
    }
}

/// A paired computer's workspaces, for matching.
public struct ComputerWorkspaces: Sendable {
    public var device: String
    public var computer: String
    public var workspaces: [Home.Workspace]

    public init(device: String, computer: String, workspaces: [Home.Workspace]) {
        self.device = device
        self.computer = computer
        self.workspaces = workspaces
    }

    /// The workspaces across `computers` whose branch carries `pull`, in
    /// the computers' order.
    public static func places(of pull: GitHubSearchPull, in computers: [ComputerWorkspaces]) -> [WorkspacePlace] {
        let wanted = pull.branch
        return computers.flatMap { computer in
            computer.workspaces.compactMap { workspace in
                guard let github = workspace.github, GitHubBranch(github) == wanted else { return nil }
                return WorkspacePlace(device: computer.device, computer: computer.computer, workspace: workspace.id)
            }
        }
    }
}

/// A branch on GitHub, as a workspace names it and a pull request's head
/// is: the repository it targets, who owns the head, and the branch.
/// GitHub's names are compared without case; branch names are not.
public struct GitHubBranch: Hashable, Sendable, CustomStringConvertible {
    public let host: GitHubHost
    public let owner: String
    public let repo: String
    public let headOwner: String
    public let branch: String

    public init(host: GitHubHost, owner: String, repo: String, headOwner: String, branch: String) {
        self.host = host
        self.owner = owner.lowercased()
        self.repo = repo.lowercased()
        self.headOwner = headOwner.lowercased()
        self.branch = branch
    }

    /// The branch a workspace is on; nil off GitHub or on a detached HEAD.
    public init?(_ repository: Home.GitHubRepository) {
        guard let host = GitHubHost(repository.host), let head = repository.headRef else { return nil }
        self.init(host: host, owner: repository.owner, repo: repository.repo, headOwner: head.owner, branch: head.branch)
    }

    public var description: String { "\(host.name)/\(owner)/\(repo)/\(headOwner):\(branch)" }
}
