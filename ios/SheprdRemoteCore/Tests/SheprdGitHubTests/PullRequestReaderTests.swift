import Foundation
import Testing
@testable import SheprdGitHub
@testable import SheprdRemoteCore

struct PullRequestReaderTests {
    private let transport = FakeTransport()
    private let lenders = FakeLenders()
    private let store = TokenStore(vault: .memory())

    private func reader() -> PullRequestReader {
        PullRequestReader(
            leases: GitHubLeases(store: store, now: { GitHubFixtures.start }),
            api: GitHubAPI(transport: transport.transport, now: { GitHubFixtures.start })
        )
    }

    private var source: Lenders { Lenders(lenders: [GitHubFixtures.lender("mac")], fetch: lenders.fetch) }
    private let computers = PullRequestChecks(number: 7, checks: [PullRequestCheck(name: "from the computer", state: .passed)])

    @Test func aLeaseReadsGitHubDirectly() async throws {
        lenders.answer("mac", GitHubFixtures.answer("t1"))
        transport.enqueue(try GitHubFixtures.response("open_pr"))
        let read = try await reader().checks(workspace: GitHubFixtures.workspace(), computer: "Mac", source: source) {
            Issue.record("the computer was asked")
            return computers
        }
        #expect(read.origin == .github)
        #expect(read.value.number == 42)
    }

    @Test func noLeaseAsksTheComputerAndSaysSo() async throws {
        let read = try await reader().checks(workspace: GitHubFixtures.workspace(), computer: "Mac", source: source) { computers }
        #expect(read.origin == .computer("Mac"))
        #expect(read.value == computers)
        #expect(transport.requests.isEmpty)
    }

    @Test func aWorkspaceWithoutARepositoryAsksTheComputer() async throws {
        lenders.answer("mac", GitHubFixtures.answer("t1"))
        let read = try await reader().checks(workspace: GitHubFixtures.workspace(github: nil), computer: "Mac", source: source) { computers }
        #expect(read.origin == .computer("Mac"))
        #expect(lenders.asked.isEmpty)
    }

    @Test(arguments: [
        GitHubResponse(status: 404),
        GitHubResponse(status: 403, headers: ["X-GitHub-SSO": "required; url=https://github.com/orgs/acme/sso"]),
        GitHubResponse(status: 403),
        GitHubResponse(status: 502),
    ])
    func aFailedReadFallsBackQuietly(answer: GitHubResponse) async throws {
        lenders.answer("mac", GitHubFixtures.answer("t1"))
        transport.enqueue(answer)
        let read = try await reader().checks(workspace: GitHubFixtures.workspace(), computer: "Mac", source: source) { computers }
        #expect(read.origin == .computer("Mac"))
        #expect(read.value == computers)
    }

    @Test func noNetworkFallsBackToo() async throws {
        lenders.answer("mac", GitHubFixtures.answer("t1"))
        let read = try await reader().checks(workspace: GitHubFixtures.workspace(), computer: "Mac", source: source) { computers }
        #expect(read.origin == .computer("Mac"))
        #expect(transport.requests.count == 1)
    }

    @Test func aRefusedTokenIsRenewedOnceBeforeFallingBack() async throws {
        lenders.answer("mac", GitHubFixtures.answer("t1"), GitHubFixtures.answer("t2"))
        transport.enqueue(GitHubResponse(status: 401))
        transport.enqueue(try GitHubFixtures.response("open_pr"))
        let read = try await reader().checks(workspace: GitHubFixtures.workspace(), computer: "Mac", source: source) { computers }
        #expect(read.origin == .github)
        #expect(transport.requests.map { $0.value(forHTTPHeaderField: "Authorization") } == ["Bearer t1", "Bearer t2"])
        #expect(store.lease(for: GitHubFixtures.host)?.token == GitHubToken("t2"))
    }

    @Test func theComputersFailureIsTheOneThrown() async throws {
        struct Away: Error {}
        await #expect(throws: Away.self) {
            try await reader().checks(workspace: GitHubFixtures.workspace(), computer: "Mac", source: source) { throw Away() }
        }
    }

    @Test func aJobLogIsReadDirectlyOrFromTheComputer() async throws {
        lenders.answer("mac", GitHubFixtures.answer("t1"))
        transport.enqueue(GitHubResponse(status: 200, body: Data("##[error]boom\n".utf8)))
        let reader = reader()
        let direct = try await reader.checkLog(job: 70, workspace: GitHubFixtures.workspace(), computer: "Mac", source: source) {
            CheckLogExcerpt(excerpt: "computer")
        }
        #expect(direct.origin == .github)
        #expect(direct.value.excerpt == "##[error]boom")
        let fallback = try await reader.checkLog(job: 71, workspace: GitHubFixtures.workspace(), computer: "Mac", source: source) {
            CheckLogExcerpt(excerpt: "computer")
        }
        #expect(fallback.origin == .computer("Mac"))
        #expect(fallback.value.excerpt == "computer")
    }

    @Test func visibleWorkspacesAreReadInOneQueryPerRepository() async throws {
        lenders.answer("mac", GitHubFixtures.answer("t1"))
        transport.enqueue(try GitHubFixtures.response("no_pr"))
        let workspaces = [
            GitHubFixtures.workspace(id: "a"),
            GitHubFixtures.workspace(id: "b", github: .init(host: "github.com", owner: "acme", repo: "app", branch: "reused")),
            GitHubFixtures.workspace(id: "c", github: nil),
        ]
        let found = await reader().pullRequests(workspaces: workspaces, source: source)
        #expect(transport.requests.count == 1)
        #expect(found["a"] == .some(nil))
        #expect(found["b"]??.number == 12)
        #expect(found["c"] == nil)
    }
}
