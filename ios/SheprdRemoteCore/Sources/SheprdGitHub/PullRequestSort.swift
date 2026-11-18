import Foundation

/// How the Pull Requests tab orders each of its lists.
public enum PullRequestSort: String, CaseIterable, Identifiable, Sendable {
    /// The most recently updated first.
    case updated
    /// What needs the person first: changes asked for, then failing
    /// checks, a review still due, checks still running, approved, and
    /// drafts last; the most recently updated first within each.
    case status

    public var id: Self { self }

    /// Where the person keeps their choice.
    public static let key = "pullRequestSort"
}

extension PullRequestLists {
    /// Both lists in `sort`'s order, their totals unchanged.
    public func sorted(_ sort: PullRequestSort) -> PullRequestLists {
        var sorted = self
        sorted.mine = Self.order(mine, sort)
        sorted.reviewRequested = Self.order(reviewRequested, sort)
        return sorted
    }

    private static func order(_ pulls: [GitHubSearchPull], _ sort: PullRequestSort) -> [GitHubSearchPull] {
        // Indices break the last ties, so the order never shuffles.
        let indexed = Array(pulls.enumerated())
        return indexed.sorted { a, b in
            if sort == .status {
                let (first, second) = (rank(a.element), rank(b.element))
                if first != second { return first < second }
            }
            switch (a.element.updatedAt, b.element.updatedAt) {
            case let (first?, second?) where first != second: return first > second
            case (_?, nil): return true
            case (nil, _?): return false
            default: return a.offset < b.offset
            }
        }
        .map(\.element)
    }

    /// Lower needs the person sooner.
    private static func rank(_ pull: GitHubSearchPull) -> Int {
        if pull.isDraft { return 5 }
        if pull.reviewDecision == .changesRequested { return 0 }
        if pull.checks.failing > 0 { return 1 }
        if pull.reviewDecision == .reviewRequired { return 2 }
        if pull.checks.pending > 0 { return 3 }
        if pull.reviewDecision == .approved { return 4 }
        return 2
    }
}
