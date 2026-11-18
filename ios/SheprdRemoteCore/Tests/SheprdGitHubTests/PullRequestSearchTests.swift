import Foundation
import Testing
@testable import SheprdGitHub
@testable import SheprdRemoteCore

/// The pull request lists: the search query, reading its answer, and which
/// token reads it.
struct PullRequestSearchTests {
    private let transport = FakeTransport()
    private let lenders = FakeLenders()
    private let vault = SecretVault.memory()

    private func reader() -> PullRequestReader {
        let now: @Sendable () -> Date = { GitHubFixtures.start }
        return PullRequestReader(
            leases: GitHubLeases(store: TokenStore(vault: .memory()), now: now),
            api: GitHubAPI(transport: transport.transport, now: now),
            signIn: GitHubSignIn(flow: GitHubDeviceFlow(transport: transport.transport, now: now), store: GitHubSignInStore(vault: vault))
        )
    }

    private func source(lending: Bool = true) -> Lenders {
        Lenders(lenders: lending ? [GitHubFixtures.lender("mac")] : [], fetch: lenders.fetch)
    }

    private var authorizations: [String?] { transport.requests.map { $0.value(forHTTPHeaderField: "Authorization") } }

    private func parse(_ name: String) throws -> PullRequestLists {
        try PullRequestSearchAnswer.parse(try GitHubFixtures.response(name), host: GitHubFixtures.host, now: GitHubFixtures.start)
    }

    @Test func theSearchesTravelAsVariables() throws {
        let body = try #require(try JSONSerialization.jsonObject(with: PullRequestSearchQuery.body) as? [String: Any])
        let variables = try #require(body["variables"] as? [String: String])
        #expect(variables["mine"]?.contains("author:@me") == true)
        #expect(variables["review"]?.contains("review-requested:@me") == true)
        #expect(variables.values.allSatisfy { $0.contains("is:pr") && $0.contains("is:open") })
        let text = try #require(body["query"] as? String)
        #expect(text.contains("first: \(PullRequestSearchQuery.limit)"))
        #expect(!text.contains("@me"))
    }

    @Test func anAnswerBecomesTwoLists() throws {
        let lists = try parse("search")
        #expect(lists.mine.map(\.number) == [1342, 214])
        #expect(lists.reviewRequested.map(\.number) == [57])
        #expect(lists.mineTotal == 3)
        #expect(lists.reviewRequestedTotal == 120)

        let first = lists.mine[0]
        #expect(first.repository == "sheprd-labs/sheprd")
        #expect(first.reviewDecision == .changesRequested)
        #expect(first.headRefName == "feat/ios-native-github")
        #expect(first.headOwner == "sheprd-labs")
        #expect(first.author == "octocat")
        #expect(first.updatedAt == (try Date("2027-01-15T08:01:00Z", strategy: .iso8601)))
        #expect(first.checks == .init(failing: 2, pending: 1, passed: 5))

        let draft = lists.mine[1]
        #expect(draft.isDraft)
        #expect(draft.reviewDecision == .none)
        #expect(draft.checks == .init())
    }

    @Test func remoteTextIsCleanedAndLinksAreBuilt() throws {
        let lists = try parse("search")
        // The right-to-left override is dropped.
        #expect(lists.mine[0].title == "Read pull requests on the phone with a lent GitHub sign-in")
        #expect(lists.mine[0].url?.absoluteString == "https://github.com/sheprd-labs/sheprd/pull/1342")
        // A repository named like a path is not listed at all.
        #expect(!lists.mine.contains { $0.number == 9 })
    }

    @Test func aDeletedForkStillCountsItsStatuses() throws {
        let review = try parse("search").reviewRequested[0]
        #expect(review.headOwner == nil)
        #expect(review.branch.headOwner == "sheprd-labs")
        #expect(review.checks == .init(failing: 1, pending: 0, passed: 6))
    }

    @Test func errorsWithoutDataAreTyped() throws {
        #expect(throws: GitHubError.forbidden) { try parse("search_forbidden") }
        #expect {
            try parse("search_rate_limited")
        } throws: { error in
            if case GitHubError.rateLimited = error { true } else { false }
        }
        #expect(throws: GitHubError.malformed) {
            try PullRequestSearchAnswer.parse(GitHubResponse(status: 200, body: Data("[]".utf8)), host: GitHubFixtures.host, now: GitHubFixtures.start)
        }
    }

    @Test func listsAreBoundedAndWithoutRepeats() throws {
        let node = #"{"number":1,"title":"t","state":"OPEN","isDraft":false,"headRefName":"b","repository":{"name":"r","owner":{"login":"o"}}}"#
        let many = (0..<80).map { node.replacingOccurrences(of: #""number":1"#, with: #""number":\#($0 % 60 + 1)"#) }
        let body = #"{"data":{"mine":{"issueCount":80,"nodes":[\#(many.joined(separator: ","))]},"review":{"issueCount":0,"nodes":[]}}}"#
        let lists = try PullRequestSearchAnswer.parse(GitHubResponse(status: 200, body: Data(body.utf8)), host: GitHubFixtures.host, now: GitHubFixtures.start)
        #expect(lists.mine.count == PullRequestSearchQuery.limit)
        #expect(Set(lists.mine.map(\.id)).count == lists.mine.count)
    }

    @Test func theOwnSignInReadsTheLists() async throws {
        try GitHubSignInStore(vault: vault).save(SignInFixtures.credential())
        transport.enqueue(try GitHubFixtures.response("search"))
        let read = await reader().pullRequestLists(source: source())
        guard case let .found(lists) = read else { Issue.record("not found: \(read)"); return }
        #expect(lists.mine.count == 2)
        #expect(authorizations == ["Bearer ghu_a1"])
        #expect(transport.requests.first?.url?.absoluteString == "https://api.github.com/graphql")
        #expect(lenders.asked.isEmpty)
    }

    @Test func signedOutALentTokenReadsThem() async throws {
        lenders.answer("mac", GitHubFixtures.answer("lent"))
        transport.enqueue(try GitHubFixtures.response("search"))
        guard case .found = await reader().pullRequestLists(source: source()) else { Issue.record("not found"); return }
        #expect(authorizations == ["Bearer lent"])
    }

    @Test func aFailingOwnSignInFallsBackToALentToken() async throws {
        try GitHubSignInStore(vault: vault).save(SignInFixtures.credential())
        lenders.answer("mac", GitHubFixtures.answer("lent"))
        transport.enqueue(GitHubResponse(status: 403))
        transport.enqueue(try GitHubFixtures.response("search"))
        guard case .found = await reader().pullRequestLists(source: source()) else { Issue.record("not found"); return }
        #expect(authorizations == ["Bearer ghu_a1", "Bearer lent"])
    }

    @Test func noAccountAnywhereSaysSoWithoutAsking() async throws {
        #expect(await reader().pullRequestLists(source: source(lending: false)) == .noAccount)
        #expect(transport.requests.isEmpty)
    }

    @Test func anEnterpriseLenderIsNoAccountForTheLists() async throws {
        let source = Lenders(lenders: [GitHubFixtures.lender("mac", hosts: ["acme.ghe.com"])], fetch: lenders.fetch)
        #expect(await reader().pullRequestLists(source: source) == .noAccount)
    }

    @Test func aTokenThatCannotReadIsAFailure() async throws {
        lenders.answer("mac", GitHubFixtures.answer("lent"))
        transport.enqueue(GitHubResponse(status: 502))
        #expect(await reader().pullRequestLists(source: source()) == .failed)
    }
}
