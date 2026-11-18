import Foundation
import Testing
@testable import SheprdGitHub
@testable import SheprdRemoteCore

/// Settings > GitHub: a section per host, from the own sign-in, the leases
/// held, and what computers say they lend.
struct GitHubAccountsTests {
    private let expiry = GitHubFixtures.start.addingTimeInterval(7 * 3600)

    private func lease(_ host: String, login: String, from computer: String, source: LeaseSource, expires: Date? = nil) -> GitHubLease {
        GitHubLease(
            host: GitHubHost(host)!, login: login, token: GitHubToken("t"), expiresAt: expires, source: source,
            lender: computer.lowercased(), lenderName: computer, received: GitHubFixtures.start
        )
    }

    @Test func githubComIsAlwaysFirstSoSigningInIsOffered() {
        let accounts = GitHubHostAccount.accounts(own: .idle, ownExpiry: nil, leases: [], lendings: [])
        #expect(accounts.map(\.host.name) == ["github.com"])
        #expect(accounts[0].signsIn)
        #expect(accounts[0].own == nil)
        #expect(accounts[0].lent == nil)
    }

    @Test func signedOutWithALentAccount() {
        let accounts = GitHubHostAccount.accounts(
            own: .idle, ownExpiry: nil,
            leases: [lease("github.com", login: "octocat", from: "MacBook Pro", source: .sheprd, expires: expiry)],
            lendings: []
        )
        #expect(accounts[0].lent == .init(login: "octocat", computer: "MacBook Pro", source: .sheprd, expiresAt: expiry))
    }

    @Test func signedInKeepsTheLentAccountAsFallback() {
        let accounts = GitHubHostAccount.accounts(
            own: .signedIn(login: "octocat", access: .installed), ownExpiry: expiry, leases: [],
            lendings: [GitHubLending(computer: "MacBook Pro", accounts: [.init(host: "github.com", login: "octocat")])]
        )
        #expect(accounts[0].own == .init(login: "octocat", expiresAt: expiry, access: .installed))
        #expect(accounts[0].lent?.computer == "MacBook Pro")
        #expect(accounts[0].lent?.source == nil)
    }

    @Test func enterpriseHostsFollowByNameAndCannotSignIn() {
        let accounts = GitHubHostAccount.accounts(
            own: .idle, ownExpiry: nil,
            leases: [lease("github.acme-corp.com", login: "octocat-acme", from: "studio", source: .gh)],
            lendings: [
                GitHubLending(computer: "build-01", accounts: [.init(host: "acme.ghe.com", login: "ana"), .init(host: "not a host", login: "x")]),
                GitHubLending(computer: "studio", accounts: [.init(host: "acme.ghe.com", login: "other")]),
            ]
        )
        #expect(accounts.map(\.host.name) == ["github.com", "acme.ghe.com", "github.acme-corp.com"])
        #expect(!accounts[1].signsIn)
        // The first computer naming a host is the one shown.
        #expect(accounts[1].lent?.computer == "build-01")
        #expect(accounts[2].lent == .init(login: "octocat-acme", computer: "studio", source: .gh, expiresAt: nil))
    }

    @Test func aLeaseTellsMoreThanAHome() {
        let accounts = GitHubHostAccount.accounts(
            own: .idle, ownExpiry: nil,
            leases: [lease("github.com", login: "octocat", from: "studio", source: .gh)],
            lendings: [GitHubLending(computer: "MacBook Pro", accounts: [.init(host: "github.com", login: "octocat")])]
        )
        #expect(accounts[0].lent?.computer == "studio")
        #expect(accounts[0].lent?.source == .gh)
    }

    @Test func aRevokedSignInIsShownAsAProblem() {
        let accounts = GitHubHostAccount.accounts(own: .failed(.refreshRejected), ownExpiry: nil, leases: [], lendings: [])
        #expect(accounts[0].problem == .refreshRejected)
        #expect(accounts[0].own == nil)
    }
}
