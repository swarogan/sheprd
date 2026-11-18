import Foundation
import Testing
@testable import SheprdGitHub

/// The Pull Requests tab's two orders, applied to each list on its own.
struct PullRequestSortTests {
    private static func pull(_ title: String, minutesAgo: Double?, review: GitHubPullRequest.ReviewDecision = .none,
                             draft: Bool = false, checks: GitHubSearchPull.CheckCounts = .init()) throws -> GitHubSearchPull {
        let host = try #require(GitHubHost("github.com"))
        return GitHubSearchPull(host: host, owner: "o", repo: "r", number: 1, title: title, isDraft: draft,
                                reviewDecision: review, updatedAt: minutesAgo.map { Date(timeIntervalSince1970: 1_000_000 - $0 * 60) },
                                headRefName: "b", checks: checks)
    }

    @Test func updatedPutsTheNewestFirstAndTheUndatedLast() throws {
        let lists = PullRequestLists(mine: [
            try Self.pull("old", minutesAgo: 90), try Self.pull("undated", minutesAgo: nil), try Self.pull("new", minutesAgo: 5),
        ])
        #expect(lists.sorted(.updated).mine.map(\.title) == ["new", "old", "undated"])
    }

    @Test func statusPutsWhatNeedsThePersonFirst() throws {
        let lists = PullRequestLists(mine: [
            try Self.pull("draft", minutesAgo: 1, review: .changesRequested, draft: true),
            try Self.pull("approved", minutesAgo: 2, review: .approved),
            try Self.pull("running", minutesAgo: 3, checks: .init(pending: 1)),
            try Self.pull("review due", minutesAgo: 4, review: .reviewRequired),
            try Self.pull("failing", minutesAgo: 5, review: .approved, checks: .init(failing: 1)),
            try Self.pull("changes", minutesAgo: 6, review: .changesRequested),
        ])
        #expect(lists.sorted(.status).mine.map(\.title) == ["changes", "failing", "review due", "running", "approved", "draft"])
    }

    @Test func statusTiesGoToTheNewest() throws {
        let lists = PullRequestLists(reviewRequested: [
            try Self.pull("older", minutesAgo: 30, review: .approved), try Self.pull("newer", minutesAgo: 3, review: .approved),
        ], reviewRequestedTotal: 9)
        let sorted = lists.sorted(.status)
        #expect(sorted.reviewRequested.map(\.title) == ["newer", "older"])
        #expect(sorted.reviewRequestedTotal == 9)
    }
}
