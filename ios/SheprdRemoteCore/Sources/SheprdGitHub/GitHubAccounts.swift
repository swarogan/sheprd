import Foundation
import SheprdRemoteCore

/// The accounts a computer's `home` says it lends, for Settings.
public struct GitHubLending: Sendable {
    public var computer: String
    public var accounts: [Home.GitHubAccount]

    public init(computer: String, accounts: [Home.GitHubAccount]) {
        self.computer = computer
        self.accounts = accounts
    }
}

/// One GitHub host in Settings > GitHub: this phone's own sign-in, which
/// only github.com has, and the account a computer lends, which reads
/// fall back to.
public struct GitHubHostAccount: Equatable, Sendable, Identifiable {
    public var host: GitHubHost
    public var own: Own?
    public var lent: Lent?
    /// This phone's sign-in stopped working, and why; it signs in again.
    public var problem: GitHubSignInError?

    public init(host: GitHubHost, own: Own? = nil, lent: Lent? = nil, problem: GitHubSignInError? = nil) {
        self.host = host
        self.own = own
        self.lent = lent
        self.problem = problem
    }

    public var id: String { host.name }

    /// Whether this phone can sign in to the host itself.
    public var signsIn: Bool { host == PullRequestReader.listHost }

    public struct Own: Equatable, Sendable {
        public var login: String
        /// When its access token expires; it renews shortly before.
        public var expiresAt: Date?
        public var access: GitHubAppAccess

        public init(login: String, expiresAt: Date?, access: GitHubAppAccess) {
            self.login = login
            self.expiresAt = expiresAt
            self.access = access
        }
    }

    public struct Lent: Equatable, Sendable {
        public var login: String
        public var computer: String
        /// Known once the phone has borrowed the token; nil while only the
        /// computer's `home` names it.
        public var source: LeaseSource?
        public var expiresAt: Date?

        public init(login: String, computer: String, source: LeaseSource?, expiresAt: Date?) {
            self.login = login
            self.computer = computer
            self.source = source
            self.expiresAt = expiresAt
        }
    }

    /// github.com first, always, so signing in is offered; then each host
    /// a computer lends, by name. A lease held tells most about a host's
    /// account; otherwise the first of `lendings` naming the host, which
    /// the caller orders most recently reached first.
    public static func accounts(
        own state: GitHubSignInState,
        ownExpiry: Date?,
        leases: [GitHubLease],
        lendings: [GitHubLending]
    ) -> [GitHubHostAccount] {
        let github = PullRequestReader.listHost
        var lent: [GitHubHost: Lent] = [:]
        for lending in lendings {
            for account in lending.accounts {
                guard let host = GitHubHost(account.host), lent[host] == nil, !account.login.isEmpty else { continue }
                lent[host] = Lent(login: String(account.login.prefix(100)), computer: lending.computer, source: nil, expiresAt: nil)
            }
        }
        for lease in leases {
            lent[lease.host] = Lent(login: lease.login, computer: lease.lenderName, source: lease.source, expiresAt: lease.expiresAt)
        }
        var own: Own?
        var problem: GitHubSignInError?
        switch state {
        case let .signedIn(login, access):
            own = Own(login: login, expiresAt: ownExpiry, access: access)
        case let .failed(error) where error == .refreshRejected:
            problem = error
        default:
            break
        }
        let others = lent.keys.filter { $0 != github }.sorted { $0.name < $1.name }
        return [GitHubHostAccount(host: github, own: own, lent: lent[github], problem: problem)]
            + others.map { GitHubHostAccount(host: $0, own: nil, lent: lent[$0]) }
    }
}
