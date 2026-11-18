import Foundation
import Testing
@testable import SheprdGitHub

/// The phone caches the lists as JSON, so they show at once on the next
/// launch: everything a row draws survives the round trip.
struct PullRequestListsCodingTests {
    @Test func listsSurviveACacheRoundTrip() throws {
        let host = try #require(GitHubHost("github.com"))
        let pull = GitHubSearchPull(
            host: host, owner: "sheprd-labs", repo: "sheprd", number: 1349, title: "Search Home",
            state: .open, isDraft: true, reviewDecision: .changesRequested, author: "octocat",
            updatedAt: Date(timeIntervalSince1970: 1_800_000_000), headRefName: "worktree/x", headOwner: "fork",
            checks: .init(failing: 1, pending: 2, passed: 3)
        )
        let lists = PullRequestLists(mine: [pull], reviewRequested: [pull], mineTotal: 7, reviewRequestedTotal: 1)
        let decoded = try JSONDecoder().decode(PullRequestLists.self, from: JSONEncoder().encode(lists))
        #expect(decoded == lists)
    }

    @Test func anUnknownStateFailsTheWholeFileRatherThanGuessing() throws {
        let host = try #require(GitHubHost("github.com"))
        let pull = GitHubSearchPull(host: host, owner: "o", repo: "r", number: 1, title: "t", headRefName: "b")
        var json = try JSONSerialization.jsonObject(with: JSONEncoder().encode(PullRequestLists(mine: [pull]))) as? [String: Any]
        var mine = json?["mine"] as? [[String: Any]]
        mine?[0]["state"] = "SOMETHING_NEW"
        json?["mine"] = mine
        let data = try JSONSerialization.data(withJSONObject: json ?? [:])
        #expect(throws: DecodingError.self) { try JSONDecoder().decode(PullRequestLists.self, from: data) }
    }
}
