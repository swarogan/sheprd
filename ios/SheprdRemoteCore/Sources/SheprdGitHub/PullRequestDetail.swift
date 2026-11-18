import Foundation

/// A pull request as its detail screen shows it, read-only: description,
/// reviewers, the conversation with review comments, and the size of the
/// change. Every text comes from GitHub and is untrusted: it is cleaned of
/// control and direction characters, bounded, and shown as Markdown or
/// plain text, never as HTML.
public struct PullRequestDetail: Hashable, Sendable {
    public var number: UInt64
    public var title: String
    /// Its page on GitHub; empty when GitHub sent one this phone will not
    /// open (anything but `https`).
    public var url: String
    /// `OWNER/REPO`.
    public var repository: String
    public var state: GitHubPullRequest.State
    public var isDraft: Bool
    public var reviewDecision: GitHubPullRequest.ReviewDecision
    public var author: String?
    /// The description, Markdown; empty when there is none.
    public var body: String
    public var baseRef: String
    public var headRef: String
    public var createdAt: Date?
    public var additions: Int
    public var deletions: Int
    public var changedFiles: Int
    public var reviewers: [Reviewer]
    /// Oldest first, as GitHub's conversation tab reads.
    public var timeline: [TimelineItem]
    /// Items before `timeline`'s first that were not read.
    public var earlierItems: Int

    public init(
        number: UInt64, title: String, url: String, repository: String, state: GitHubPullRequest.State, isDraft: Bool,
        reviewDecision: GitHubPullRequest.ReviewDecision, author: String?, body: String, baseRef: String, headRef: String,
        createdAt: Date?, additions: Int, deletions: Int, changedFiles: Int, reviewers: [Reviewer], timeline: [TimelineItem],
        earlierItems: Int
    ) {
        self.number = number
        self.title = title
        self.url = url
        self.repository = repository
        self.state = state
        self.isDraft = isDraft
        self.reviewDecision = reviewDecision
        self.author = author
        self.body = body
        self.baseRef = baseRef
        self.headRef = headRef
        self.createdAt = createdAt
        self.additions = additions
        self.deletions = deletions
        self.changedFiles = changedFiles
        self.reviewers = reviewers
        self.timeline = timeline
        self.earlierItems = earlierItems
    }

    /// What a review says.
    public enum Verdict: String, Hashable, Sendable {
        case approved = "APPROVED"
        case changesRequested = "CHANGES_REQUESTED"
        case commented = "COMMENTED"
        case dismissed = "DISMISSED"
        case pending = "PENDING"
    }

    /// Someone asked to review, or who reviewed: their latest verdict, or
    /// none while the request is still open.
    public struct Reviewer: Hashable, Sendable, Identifiable {
        /// A login, or `ORG/TEAM` for a team.
        public var name: String
        public var isTeam: Bool
        public var verdict: Verdict?

        public init(name: String, isTeam: Bool = false, verdict: Verdict? = nil) {
            self.name = name
            self.isTeam = isTeam
            self.verdict = verdict
        }

        public var id: String { name }
    }

    /// A comment a review left on a line of a file.
    public struct ReviewComment: Hashable, Sendable {
        public var path: String
        /// The line in the file as it is now, or as it was when the
        /// comment was written when that line is gone.
        public var line: Int?
        public var body: String

        public init(path: String, line: Int?, body: String) {
            self.path = path
            self.line = line
            self.body = body
        }
    }

    public struct TimelineItem: Hashable, Sendable, Identifiable {
        public var id: String
        /// Who did it; nil for a deleted account.
        public var actor: String?
        public var date: Date?
        public var kind: Kind

        public init(id: String, actor: String?, date: Date?, kind: Kind) {
            self.id = id
            self.actor = actor
            self.date = date
            self.kind = kind
        }

        public enum Kind: Hashable, Sendable {
            case comment(body: String)
            /// `comments` are the review's first file comments; `more`
            /// counts those left out.
            case review(verdict: Verdict, body: String, comments: [ReviewComment], more: Int)
            case reviewRequested(reviewer: String)
            /// A commit pushed: its short id and first line.
            case commit(oid: String, headline: String)
            case forcePushed(oid: String?)
            case readyForReview
            case closed
            case reopened
            case merged
        }
    }
}
