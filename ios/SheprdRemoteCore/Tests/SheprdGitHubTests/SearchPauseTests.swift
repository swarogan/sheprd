import Foundation
import Testing
@testable import SheprdGitHub
@testable import SheprdRemoteCore

/// GitHub asking to wait inside a 200 answer pauses the host for the
/// lists, as it does for branch and detail reads.
struct SearchPauseTests {
    private let transport = FakeTransport()
    private let lenders = FakeLenders()
    private let vault = SecretVault.memory()
    private let clock = TestClock()

    private func reader() -> PullRequestReader {
        PullRequestReader(
            leases: GitHubLeases(store: TokenStore(vault: .memory()), now: clock.read),
            api: GitHubAPI(transport: transport.transport, now: clock.read),
            signIn: GitHubSignIn(flow: GitHubDeviceFlow(transport: transport.transport, now: clock.read), store: GitHubSignInStore(vault: vault))
        )
    }

    private var source: Lenders { Lenders(lenders: [GitHubFixtures.lender("mac")], fetch: lenders.fetch) }

    @Test func aRateLimitedSearchPausesTheHostAndAsksNoLender() async throws {
        try GitHubSignInStore(vault: vault).save(SignInFixtures.credential())
        lenders.answer("mac", GitHubFixtures.answer("lent"))
        transport.enqueue(try GitHubFixtures.response("search_rate_limited"))
        let reader = reader()
        #expect(await reader.pullRequestLists(source: source) == .failed)
        #expect(transport.requests.count == 1)
        #expect(lenders.asked.isEmpty)
        // Paused: the next read sends nothing until the wait is over.
        #expect(await reader.pullRequestLists(source: source) == .failed)
        #expect(transport.requests.count == 1)
        clock.advance(GitHubHTTP.defaultWait + 1)
        transport.enqueue(try GitHubFixtures.response("search"))
        guard case .found = await reader.pullRequestLists(source: source) else { Issue.record("not found"); return }
        #expect(transport.requests.count == 2)
    }

    @Test func theSearchItselfPausesTheAPI() async throws {
        let api = GitHubAPI(transport: transport.transport, now: clock.read)
        transport.enqueue(try GitHubFixtures.response("search_rate_limited"))
        await #expect(throws: GitHubError.self) { try await api.searchPullRequests(host: GitHubFixtures.host, token: GitHubToken("t")) }
        #expect(await api.isPaused(GitHubFixtures.host))
    }
}
