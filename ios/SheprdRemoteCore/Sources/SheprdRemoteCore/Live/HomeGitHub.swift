import Foundation

extension Home {
    /// A GitHub account a computer lends this phone: `github_hosts` in its
    /// `home`.
    public struct GitHubAccount: Codable, Hashable, Sendable {
        public var host: String
        public var login: String

        public init(host: String, login: String) {
            self.host = host
            self.login = login
        }
    }

    /// Where a workspace's pull request lives: a fork with an `upstream`
    /// remote names the upstream repository.
    public struct GitHubRepository: Codable, Hashable, Sendable {
        public var host: String
        public var owner: String
        public var repo: String
        /// The branch its pull request's head is; absent on a detached HEAD.
        public var branch: String?
        /// The checkout's HEAD commit (40 or 64 hex digits), as the computer
        /// reports it; absent on an empty repository.
        public var head: String?
        /// Who owns the repository `branch` is pushed to, when that is not
        /// `owner`: a fork whose pull request targets the upstream named
        /// here. Absent from older computers, which read as `owner`.
        public var headOwner: String?

        enum CodingKeys: String, CodingKey {
            case host, owner, repo, branch, head
            case headOwner = "head_owner"
        }

        public init(host: String, owner: String, repo: String, branch: String? = nil, head: String? = nil, headOwner: String? = nil) {
            self.host = host
            self.owner = owner
            self.repo = repo
            self.branch = branch
            self.head = head
            self.headOwner = headOwner
        }

        /// Who owns the pull request's head, and its branch: the fork in
        /// `headOwner`, else this repository's owner, and `branch`.
        public var headRef: (owner: String, branch: String)? {
            guard let branch, !branch.isEmpty else { return nil }
            let fork = headOwner.flatMap { $0.isEmpty ? nil : $0 }
            return (fork ?? owner, branch)
        }
    }
}
