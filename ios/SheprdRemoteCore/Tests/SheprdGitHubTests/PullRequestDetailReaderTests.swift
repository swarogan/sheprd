import Foundation
import Testing
@testable import SheprdGitHub
@testable import SheprdRemoteCore

struct PullRequestDetailReaderTests {
    private let transport = FakeTransport()
    private let lenders = FakeLenders()
    private let store = TokenStore(vault: .memory())
    private let target = PullRequestTarget(host: GitHubFixtures.host, owner: "acme", repo: "app", number: 42)

    private func api() -> GitHubAPI {
        GitHubAPI(transport: transport.transport, now: { GitHubFixtures.start })
    }

    private func reader() -> PullRequestReader {
        PullRequestReader(leases: GitHubLeases(store: store, now: { GitHubFixtures.start }), api: api())
    }

    private var source: Lenders { Lenders(lenders: [GitHubFixtures.lender("mac")], fetch: lenders.fetch) }

    private func detail(_ api: GitHubAPI) async throws -> PullRequestDetail {
        try await api.pullRequestDetail(host: target.host, owner: "acme", repo: "app", number: 42, token: GitHubToken("t"))
    }

    @Test func aMissingPullRequestIsNotFound() async throws {
        transport.enqueue(try GitHubFixtures.response("pr_detail_missing"))
        await #expect(throws: GitHubError.notFound) { try await detail(api()) }
    }

    @Test func aMissingRepositoryIsNotFound() async throws {
        transport.enqueue(try GitHubFixtures.response("not_found"))
        await #expect(throws: GitHubError.notFound) { try await detail(api()) }
    }

    @Test func refusalsMapAsForOtherReads() async throws {
        transport.enqueue(GitHubResponse(status: 401))
        await #expect(throws: GitHubError.unauthorized) { try await detail(api()) }
        transport.enqueue(GitHubResponse(status: 403, headers: ["X-GitHub-SSO": "required; url=https://github.com/orgs/acme/sso"]))
        await #expect(throws: GitHubError.ssoRequired) { try await detail(api()) }
        transport.enqueue(GitHubResponse(status: 200, body: Data("not json".utf8)))
        await #expect(throws: GitHubError.malformed) { try await detail(api()) }
    }

    @Test func aRateLimitedAnswerPausesTheHost() async throws {
        let limited = GitHubResponse(
            status: 200, headers: ["Retry-After": "120"],
            body: Data(#"{"data":null,"errors":[{"type":"RATE_LIMITED","message":"API rate limit exceeded"}]}"#.utf8)
        )
        transport.enqueue(limited)
        let api = api()
        let until = GitHubFixtures.start.addingTimeInterval(120)
        await #expect(throws: GitHubError.rateLimited(until: until)) { try await detail(api) }
        // Nothing more is sent before then.
        await #expect(throws: GitHubError.rateLimited(until: until)) { try await detail(api) }
        #expect(transport.requests.count == 1)
    }

    @Test func aLeaseReadsTheDetailAndFiles() async throws {
        lenders.answer("mac", GitHubFixtures.answer("t1"))
        transport.enqueue(try GitHubFixtures.response("pr_detail"))
        transport.enqueue(try PullRequestFilesTests.page(2))
        let read = try #require(await reader().detail(target, source: source))
        #expect(read.detail.number == 42)
        #expect(read.files?.files.count == 2)
        #expect(transport.requests.map { $0.url?.path() } == ["/graphql", "/repos/acme/app/pulls/42/files"])
    }

    @Test func filesThatFailStillShowTheConversation() async throws {
        lenders.answer("mac", GitHubFixtures.answer("t1"))
        transport.enqueue(try GitHubFixtures.response("pr_detail"))
        transport.enqueue(GitHubResponse(status: 502))
        let read = try #require(await reader().detail(target, source: source))
        #expect(read.files == nil)
    }

    @Test func noTokenReadsNothing() async throws {
        #expect(await reader().detail(target, source: source) == nil)
        #expect(transport.requests.isEmpty)
    }

    @Test func aRefusedTokenIsRenewedOnce() async throws {
        lenders.answer("mac", GitHubFixtures.answer("t1"), GitHubFixtures.answer("t2"))
        transport.enqueue(GitHubResponse(status: 401))
        transport.enqueue(try GitHubFixtures.response("pr_detail"))
        transport.enqueue(try PullRequestFilesTests.page(1))
        let read = await reader().detail(target, source: source)
        #expect(read?.detail.number == 42)
        #expect(transport.requests.map { $0.value(forHTTPHeaderField: "Authorization") } == ["Bearer t1", "Bearer t2", "Bearer t2"])
    }

    @Test func theBranchHeaderComesFromGitHubWithItsTitle() async throws {
        lenders.answer("mac", GitHubFixtures.answer("t1"))
        transport.enqueue(try GitHubFixtures.response("open_pr"))
        let read = try await reader().branchPullRequest(workspace: GitHubFixtures.workspace(), computer: "Mac", source: source) {
            Issue.record("the computer was asked")
            return PullRequestChecks(checks: [])
        }
        #expect(read.origin == .github)
        #expect(read.value.pullRequest?.number == 42)
        #expect(read.value.checks.number == 42)
    }

    @Test func theBranchHeaderFallsBackToTheComputersChecks() async throws {
        let computers = PullRequestChecks(number: 7, checks: [PullRequestCheck(name: "lint", state: .failed, job: 1)])
        let read = try await reader().branchPullRequest(workspace: GitHubFixtures.workspace(), computer: "Mac", source: source) { computers }
        #expect(read.origin == .computer("Mac"))
        #expect(read.value == BranchPullRequest(checks: computers))
    }

    @Test func aTargetNeedsAKnownRepository() {
        #expect(PullRequestTarget(workspace: GitHubFixtures.workspace(), number: 3) == PullRequestTarget(
            host: GitHubFixtures.host, owner: "acme", repo: "app", number: 3
        ))
        #expect(PullRequestTarget(workspace: GitHubFixtures.workspace(github: nil), number: 3) == nil)
    }
}
