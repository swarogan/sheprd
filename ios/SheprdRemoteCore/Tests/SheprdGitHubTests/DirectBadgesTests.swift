import Foundation
import Testing
@testable import SheprdGitHub
@testable import SheprdRemoteCore

/// Workspace badges come from GitHub when the phone read them, from the
/// computer otherwise.
struct DirectBadgesTests {
    private func workspace(_ id: String, branch: String? = "feature", pullRequest: UInt64? = nil, github: Bool = true) -> Home.Workspace {
        var workspace = GitHubFixtures.workspace(id: id, github: github ? .init(host: "github.com", owner: "acme", repo: "app", branch: branch) : nil)
        workspace.pullRequest = pullRequest.map { Home.PullRequest(number: $0, url: "https://github.com/acme/app/pull/\($0)", tone: .waiting) }
        return workspace
    }

    private let branch = GitHubBranch(host: GitHubFixtures.host, owner: "acme", repo: "app", headOwner: "acme", branch: "feature")

    @Test func aBranchOnSeveralComputersIsAskedOnce() {
        let (asked, branches) = DirectBadges.requests([workspace("w"), workspace("w"), workspace("v"), workspace("x", branch: "other")])
        #expect(asked.count == 2)
        #expect(Set(asked.map(\.id)) == Set(branches.keys))
        #expect(branches[asked[0].id] == branch)
    }

    @Test func workspacesGitHubCannotBeAskedAboutAreLeftOut() {
        let (asked, _) = DirectBadges.requests([workspace("a", github: false), workspace("b", branch: nil)])
        #expect(asked.isEmpty)
    }

    @Test func requestsAreBounded() {
        let many = (0..<(DirectBadges.limit + 20)).map { workspace("w\($0)", branch: "b\($0)") }
        #expect(DirectBadges.requests(many).workspaces.count == DirectBadges.limit)
    }

    @Test func gitHubsAnswerWinsAndNamesItsSource() throws {
        let response = try GitHubFixtures.response("open_pr")
        let query = PullRequestQuery(owner: "acme", repo: "app", branches: ["feature"])
        let read = try PullRequestAnswer.parse(response, query: query, headOwner: { _ in "acme" }, host: GitHubFixtures.host, now: GitHubFixtures.start)
        let pullRequest = try #require(try read["feature"]?.get())
        let badges = DirectBadges.answered([branch.description: pullRequest], branches: [branch.description: branch])
        #expect(badges.badge(for: workspace("w", pullRequest: 7)) == .github(pullRequest.badge))
        #expect(badges.badge(for: workspace("w", pullRequest: 7)).pullRequest?.number == pullRequest.number)
    }

    @Test func gitHubSayingNoneHidesAStaleComputerBadge() {
        let badges = DirectBadges.answered([branch.description: GitHubPullRequest?.none], branches: [branch.description: branch])
        #expect(badges.badge(for: workspace("w", pullRequest: 7)) == .github(nil))
    }

    @Test func unansweredWorkspacesKeepTheComputersBadge() {
        let badges = DirectBadges()
        #expect(badges.badge(for: workspace("w", pullRequest: 7)).pullRequest?.number == 7)
        #expect(badges.badge(for: workspace("w", pullRequest: 7, github: false)) == .computer(workspace("w", pullRequest: 7).pullRequest))
    }

    @Test func answersForUnknownIdsAreIgnored() {
        let badges = DirectBadges.answered(["elsewhere": GitHubPullRequest?.none], branches: [:])
        #expect(badges.answers.isEmpty)
    }

    @Test func theReaderAnswersByTheRequestsIds() async throws {
        let transport = FakeTransport([try GitHubFixtures.response("open_pr")])
        let lenders = FakeLenders()
        lenders.answer("mac", GitHubFixtures.answer("t1"))
        let reader = PullRequestReader(
            leases: GitHubLeases(store: TokenStore(vault: .memory()), now: { GitHubFixtures.start }),
            api: GitHubAPI(transport: transport.transport, now: { GitHubFixtures.start })
        )
        let (asked, branches) = DirectBadges.requests([workspace("w"), workspace("w")])
        let found = await reader.pullRequests(workspaces: asked, source: Lenders(lenders: [GitHubFixtures.lender("mac")], fetch: lenders.fetch))
        let badges = DirectBadges.answered(found, branches: branches)
        guard case let .github(badge) = badges.badge(for: workspace("w", pullRequest: 7)) else { Issue.record("the computer's badge"); return }
        #expect(badge?.number == 42)
        #expect(transport.requests.count == 1)
    }
}
