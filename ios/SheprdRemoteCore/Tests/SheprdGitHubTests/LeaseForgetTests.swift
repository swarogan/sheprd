import Foundation
import Testing
@testable import SheprdGitHub
@testable import SheprdRemoteCore

/// Forgetting a computer while its answer is on the way.
struct LeaseForgetTests {
    @Test func anAnswerFromAComputerForgottenMeanwhileIsNotKept() async throws {
        let store = TokenStore(vault: .memory())
        let leases = GitHubLeases(store: store, now: TestClock().read)
        let fetch: FetchLease = { device, _ in
            await leases.forget(lender: device)
            return GitHubFixtures.answer("t1", expiresIn: 8 * 3600)
        }
        await #expect(throws: LeaseError.self) {
            try await leases.lease(for: GitHubFixtures.host, lenders: [GitHubFixtures.lender("mac")], fetch: fetch)
        }
        #expect(store.lease(for: GitHubFixtures.host) == nil)
        #expect(await leases.all().isEmpty)
    }
}
