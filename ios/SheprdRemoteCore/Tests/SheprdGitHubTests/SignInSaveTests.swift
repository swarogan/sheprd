import Foundation
import Synchronization
import Testing
@testable import SheprdGitHub
@testable import SheprdRemoteCore

/// GitHub invalidates the old pair as it rotates it, so a rotated pair the
/// Keychain refuses is still used, and saved once the Keychain takes it.
struct SignInSaveTests {
    /// A Keychain that refuses writes while `refusing` is set.
    private final class FlakyVault: Sendable {
        let items = Mutex<[String: Data]>([:])
        let refusing = Mutex(false)

        var vault: SecretVault {
            SecretVault(
                read: { account in self.items.withLock { $0[account] } },
                write: { data, account in
                    if self.refusing.withLock({ $0 }) { throw Keychain.Failure(status: -25308) }
                    self.items.withLock { $0[account] = data }
                },
                delete: { account in self.items.withLock { $0[account] = nil } },
                accounts: { self.items.withLock { Array($0.keys) } }
            )
        }
    }

    private let clock = TestClock()
    private let transport = FakeTransport()
    private let keychain = FlakyVault()
    private var store: GitHubSignInStore { GitHubSignInStore(vault: keychain.vault) }

    private func signIn() -> GitHubSignIn {
        GitHubSignIn(flow: GitHubDeviceFlow(transport: transport.transport, now: clock.read), store: store)
    }

    @Test func aRotatedPairTheKeychainRefusesIsStillUsed() async throws {
        try store.save(SignInFixtures.credential(expiresIn: 60))
        keychain.refusing.withLock { $0 = true }
        transport.enqueue(SignInFixtures.token("ghu_a2", refresh: "ghr_r2"))
        let signIn = signIn()
        // The old token stopped working when GitHub rotated it.
        #expect(try await signIn.token() == GitHubToken("ghu_a2"))
        #expect(try await signIn.token() == GitHubToken("ghu_a2"))
        #expect(transport.requests.count == 1)
        #expect(try store.load()?.accessToken == GitHubToken("ghu_a1"))
    }

    @Test func aRefusedSaveIsRetriedAndKeptOnceTheKeychainTakesIt() async throws {
        try store.save(SignInFixtures.credential(expiresIn: 60))
        keychain.refusing.withLock { $0 = true }
        transport.enqueue(SignInFixtures.token("ghu_a2", refresh: "ghr_r2"))
        let signIn = signIn()
        #expect(try await signIn.token() == GitHubToken("ghu_a2"))
        keychain.refusing.withLock { $0 = false }
        #expect(try await signIn.token() == GitHubToken("ghu_a2"))
        let kept = try #require(try store.load())
        #expect(kept.accessToken == GitHubToken("ghu_a2"))
        #expect(kept.refreshToken == GitHubToken("ghr_r2"))
        #expect(transport.requests.count == 1)
    }

    @Test func aRefusedSaveIsRetriedByTheNextRenewal() async throws {
        try store.save(SignInFixtures.credential())
        keychain.refusing.withLock { $0 = true }
        transport.enqueue(SignInFixtures.token("ghu_a2", refresh: "ghr_r2"))
        transport.enqueue(SignInFixtures.token("ghu_a3", refresh: "ghr_r3"))
        let signIn = signIn()
        #expect(try await signIn.renew(rejected: GitHubToken("ghu_a1")) == GitHubToken("ghu_a2"))
        keychain.refusing.withLock { $0 = false }
        #expect(try await signIn.renew(rejected: GitHubToken("ghu_a2")) == GitHubToken("ghu_a3"))
        // The second renewal spent the rotated refresh token, not the old one.
        #expect(SignInFixtures.form(transport.requests[1])["refresh_token"] == "ghr_r2")
        #expect(try store.load()?.refreshToken == GitHubToken("ghr_r3"))
    }

    @Test func aRenewalThatFailsBeforeRotatingKeepsTheOldToken() async throws {
        try store.save(SignInFixtures.credential(expiresIn: 60))
        transport.enqueue(GitHubResponse(status: 502))
        #expect(try await signIn().token() == GitHubToken("ghu_a1"))
    }
}
