import Foundation
import Testing
@testable import SheprdGitHub
@testable import SheprdRemoteCore

/// A token refused between the conversation and its files is renewed, and
/// both are read again with the new one.
struct PullRequestFilesRetryTests {
    @Test func aTokenRefusedForTheFilesIsRenewedAndTheFilesRead() async throws {
        let transport = FakeTransport()
        let lenders = FakeLenders()
        let api = GitHubAPI(transport: transport.transport, now: { GitHubFixtures.start })
        let reader = PullRequestReader(leases: GitHubLeases(store: TokenStore(vault: .memory()), now: { GitHubFixtures.start }), api: api)
        let source = Lenders(lenders: [GitHubFixtures.lender("mac")], fetch: lenders.fetch)
        let target = PullRequestTarget(host: GitHubFixtures.host, owner: "acme", repo: "app", number: 42)
        lenders.answer("mac", GitHubFixtures.answer("t1"), GitHubFixtures.answer("t2"))
        transport.enqueue(try GitHubFixtures.response("pr_detail"))
        transport.enqueue(GitHubResponse(status: 401))
        transport.enqueue(try GitHubFixtures.response("pr_detail"))
        transport.enqueue(try PullRequestFilesTests.page(2))
        let read = try #require(await reader.detail(target, source: source))
        #expect(read.files?.files.count == 2)
        #expect(transport.requests.map { $0.value(forHTTPHeaderField: "Authorization") } == ["Bearer t1", "Bearer t1", "Bearer t2", "Bearer t2"])
    }
}
