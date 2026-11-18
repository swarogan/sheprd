import Foundation
import Security
import Testing
@testable import SheprdGitHub
@testable import SheprdRemoteCore

struct TokenStoreTests {
    private func lease(_ host: String, lender: String = "mac") -> GitHubLease {
        GitHubLease(host: GitHubHost(host)!, login: "ana", token: GitHubToken("t-\(host)"), expiresAt: GitHubFixtures.start, source: .gh, lender: lender, lenderName: "Mac", received: GitHubFixtures.start)
    }

    @Test func leasesRoundTripOnePerHost() throws {
        let store = TokenStore(vault: .memory())
        try store.save(lease("github.com"))
        try store.save(lease("acme.ghe.com", lender: "studio"))
        #expect(store.lease(for: GitHubHost("github.com")!) == lease("github.com"))
        #expect(store.all().map(\.host.name) == ["acme.ghe.com", "github.com"])
        store.forget(lender: "studio")
        #expect(store.all().map(\.host.name) == ["github.com"])
        try store.forget(GitHubHost("github.com")!)
        #expect(store.all().isEmpty)
    }

    @Test func anItemThisVersionCannotReadIsDropped() throws {
        let vault = SecretVault.memory()
        try vault.write(Data("not json".utf8), "github.com")
        #expect(TokenStore(vault: vault).lease(for: GitHubHost("github.com")!) == nil)
        #expect(try vault.read("github.com") == nil)
    }

    /// The real Keychain, where the test runner may use it: a Mac's data
    /// protection keychain needs an entitlement a `swift test` runner has
    /// not, which is the one failure accepted.
    @Test func theKeychainKeepsALeaseWhereAvailable() throws {
        let vault = Keychain.vault(service: "so.pen.herdr.remote.github.tests.\(UUID().uuidString)")
        let store = TokenStore(vault: vault)
        do {
            try store.save(lease("github.com"))
        } catch let failure as Keychain.Failure where failure.status == errSecMissingEntitlement {
            return
        }
        defer { try? store.forget(GitHubHost("github.com")!) }
        #expect(store.lease(for: GitHubHost("github.com")!) == lease("github.com"))
        #expect(try vault.accounts() == ["github.com"])
    }
}
