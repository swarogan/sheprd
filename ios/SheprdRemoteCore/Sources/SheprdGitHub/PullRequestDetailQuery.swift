import Foundation

/// The GraphQL query for one pull request's detail, and reading its answer.
/// Every list is bounded: the conversation's latest `timelineLimit` items,
/// `reviewCommentLimit` file comments per review, `reviewerLimit`
/// reviewers. Names and the number travel as variables.
struct PullRequestDetailQuery: Sendable, Equatable {
    static let timelineLimit = 50
    static let reviewCommentLimit = 10
    static let reviewerLimit = 20
    /// Characters kept of the description, and of each comment.
    static let bodyLimit = 64 * 1024
    static let commentLimit = 16 * 1024

    var owner: String
    var repo: String
    var number: UInt64

    init(owner: String, repo: String, number: UInt64) {
        self.owner = owner
        self.repo = repo
        self.number = number
    }

    var body: Data {
        get throws {
            let variables: [String: Any] = ["owner": owner, "repo": repo, "number": Int(clamping: number)]
            return try JSONSerialization.data(withJSONObject: ["query": Self.text, "variables": variables], options: [.sortedKeys])
        }
    }

    static let reviewee = """
    __typename ... on User { login } ... on Bot { login } ... on Mannequin { login } ... on Team { slug organization { login } }
    """

    static let text = """
    query($owner: String!, $repo: String!, $number: Int!) {
      repository(owner: $owner, name: $repo) {
        nameWithOwner
        pullRequest(number: $number) {
          number title url state isDraft reviewDecision body createdAt
          additions deletions changedFiles baseRefName headRefName
          author { login }
          reviewRequests(first: \(reviewerLimit)) { nodes { requestedReviewer { \(reviewee) } } }
          latestReviews(first: \(reviewerLimit)) { nodes { state author { login } } }
          timelineItems(last: \(timelineLimit), itemTypes: [ISSUE_COMMENT, PULL_REQUEST_REVIEW, REVIEW_REQUESTED_EVENT, \
    PULL_REQUEST_COMMIT, HEAD_REF_FORCE_PUSHED_EVENT, READY_FOR_REVIEW_EVENT, CLOSED_EVENT, REOPENED_EVENT, MERGED_EVENT]) {
            totalCount
            pageInfo { hasPreviousPage }
            nodes {
              __typename
              ... on IssueComment { id author { login } createdAt body }
              ... on PullRequestReview {
                id author { login } createdAt submittedAt state body
                comments(first: \(reviewCommentLimit)) { totalCount nodes { path line originalLine body } }
              }
              ... on ReviewRequestedEvent { id actor { login } createdAt requestedReviewer { \(reviewee) } }
              ... on PullRequestCommit { id commit { abbreviatedOid messageHeadline committedDate author { user { login } name } } }
              ... on HeadRefForcePushedEvent { id actor { login } createdAt afterCommit { abbreviatedOid } }
              ... on ReadyForReviewEvent { id actor { login } createdAt }
              ... on ClosedEvent { id actor { login } createdAt }
              ... on ReopenedEvent { id actor { login } createdAt }
              ... on MergedEvent { id actor { login } createdAt }
            }
          }
        }
      }
    }
    """
}

/// Reading a `PullRequestDetailQuery`'s answer.
enum PullRequestDetailAnswer {
    static func parse(_ response: GitHubResponse, now: Date) throws(GitHubError) -> PullRequestDetail {
        guard let answer = try? JSONDecoder().decode(Answer.self, from: response.body) else { throw .malformed }
        guard let repository = answer.data?.repository else {
            throw PullRequestAnswer.failure(answer.errors ?? [], response: response, now: now)
        }
        guard let node = repository.pullRequest else {
            let errors = answer.errors ?? []
            throw errors.isEmpty ? .notFound : PullRequestAnswer.failure(errors, response: response, now: now)
        }
        guard node.number > 0 else { throw .malformed }
        return node.detail(repository: PullRequestAnswer.clean(repository.nameWithOwner ?? ""))
    }

    struct Answer: Decodable {
        var data: DataField?
        var errors: [PullRequestAnswer.Answer.Failure]?

        struct DataField: Decodable {
            var repository: Repository?
        }

        struct Repository: Decodable {
            var nameWithOwner: String?
            var pullRequest: Node?
        }
    }

    typealias Login = PullRequestAnswer.Login

    /// A user, bot, mannequin, or team asked to review.
    struct Reviewee: Decodable {
        var __typename: String?
        var login: String?
        var slug: String?
        var organization: Login?

        var reviewer: PullRequestDetail.Reviewer? {
            if __typename == "Team", let slug {
                let name = organization.map { "\($0.login)/\(slug)" } ?? slug
                return .init(name: PullRequestAnswer.clean(name), isTeam: true)
            }
            return login.map { .init(name: PullRequestAnswer.clean($0)) }
        }
    }

    struct Nodes<Element: Decodable>: Decodable {
        var nodes: [Element?]?
        var all: [Element] { (nodes ?? []).compactMap { $0 } }
    }

    struct Node: Decodable {
        var number: UInt64
        var title: String
        var url: String
        var state: String
        var isDraft: Bool?
        var reviewDecision: String?
        var body: String?
        var createdAt: String?
        var additions: Int?
        var deletions: Int?
        var changedFiles: Int?
        var baseRefName: String?
        var headRefName: String?
        var author: Login?
        var reviewRequests: Nodes<Request>?
        var latestReviews: Nodes<LatestReview>?
        var timelineItems: Timeline?

        struct Request: Decodable {
            var requestedReviewer: Reviewee?
        }

        struct LatestReview: Decodable {
            var state: String
            var author: Login?
        }

        struct Timeline: Decodable {
            var totalCount: Int?
            var pageInfo: PageInfo?
            var nodes: [Item?]?
        }

        struct PageInfo: Decodable {
            var hasPreviousPage: Bool
        }

        func detail(repository: String) -> PullRequestDetail {
            let items = (timelineItems?.nodes ?? []).compactMap { $0 }
            let earlier = timelineItems?.pageInfo?.hasPreviousPage == true
                ? max((timelineItems?.totalCount ?? 0) - items.count, 0) : 0
            return PullRequestDetail(
                number: number,
                title: PullRequestAnswer.clean(title),
                url: PullRequestAnswer.linkable(url) ? url : "",
                repository: repository,
                state: GitHubPullRequest.State(rawValue: state) ?? .unknown,
                isDraft: isDraft ?? false,
                reviewDecision: reviewDecision.flatMap(GitHubPullRequest.ReviewDecision.init(rawValue:)) ?? .none,
                author: author.map { PullRequestAnswer.clean($0.login) },
                body: RemoteText.body(body ?? "", limit: PullRequestDetailQuery.bodyLimit),
                baseRef: PullRequestAnswer.clean(baseRefName ?? ""),
                headRef: PullRequestAnswer.clean(headRefName ?? ""),
                createdAt: RemoteText.date(createdAt),
                additions: max(additions ?? 0, 0),
                deletions: max(deletions ?? 0, 0),
                changedFiles: max(changedFiles ?? 0, 0),
                reviewers: reviewers,
                timeline: items.compactMap(\.item),
                earlierItems: earlier
            )
        }

        /// Who reviewed, with their latest verdict, then who is asked and
        /// has not answered since. The author is left out, as GitHub does.
        var reviewers: [PullRequestDetail.Reviewer] {
            var found: [PullRequestDetail.Reviewer] = []
            for review in latestReviews?.all ?? [] {
                guard let login = review.author?.login, login != author?.login,
                      let verdict = PullRequestDetail.Verdict(rawValue: review.state), verdict != .pending
                else { continue }
                found.append(.init(name: PullRequestAnswer.clean(login), verdict: verdict))
            }
            for request in reviewRequests?.all ?? [] {
                guard let reviewer = request.requestedReviewer?.reviewer else { continue }
                // Asked again after reviewing: waiting on them once more.
                if let index = found.firstIndex(where: { $0.name == reviewer.name }) {
                    found[index].verdict = nil
                } else {
                    found.append(reviewer)
                }
            }
            return Array(found.prefix(PullRequestDetailQuery.reviewerLimit))
        }
    }

    /// One conversation item; the fields its type has, the rest absent.
    struct Item: Decodable {
        var __typename: String
        var id: String?
        var author: Login?
        var actor: Login?
        var createdAt: String?
        var submittedAt: String?
        var body: String?
        var state: String?
        var comments: Comments?
        var requestedReviewer: Reviewee?
        var afterCommit: Oid?
        var commit: Commit?

        struct Comments: Decodable {
            var totalCount: Int?
            var nodes: [Comment?]?
        }

        struct Comment: Decodable {
            var path: String?
            var line: Int?
            var originalLine: Int?
            var body: String?
        }

        struct Oid: Decodable {
            var abbreviatedOid: String?
        }

        struct Commit: Decodable {
            var abbreviatedOid: String?
            var messageHeadline: String?
            var committedDate: String?
            var author: Author?

            struct Author: Decodable {
                var user: Login?
                var name: String?
            }
        }

        var item: PullRequestDetail.TimelineItem? {
            let who = (author ?? actor).map { PullRequestAnswer.clean($0.login) }
            let when = RemoteText.date(submittedAt ?? createdAt)
            let comment = { (text: String?) in RemoteText.body(text ?? "", limit: PullRequestDetailQuery.commentLimit) }
            let kind: PullRequestDetail.TimelineItem.Kind
            switch __typename {
            case "IssueComment":
                kind = .comment(body: comment(body))
            case "PullRequestReview":
                guard let verdict = state.flatMap(PullRequestDetail.Verdict.init(rawValue:)), verdict != .pending else { return nil }
                let notes = (comments?.nodes ?? []).compactMap { $0 }.prefix(PullRequestDetailQuery.reviewCommentLimit).map { note in
                    PullRequestDetail.ReviewComment(
                        path: String(PullRequestAnswer.clean(note.path ?? "").prefix(512)),
                        line: note.line ?? note.originalLine,
                        body: comment(note.body)
                    )
                }
                let text = comment(body)
                // A reply in a thread is a review of its own with no text.
                if verdict == .commented, text.isEmpty, notes.isEmpty { return nil }
                let more = max((comments?.totalCount ?? notes.count) - notes.count, 0)
                kind = .review(verdict: verdict, body: text, comments: notes, more: more)
            case "ReviewRequestedEvent":
                guard let reviewer = requestedReviewer?.reviewer else { return nil }
                kind = .reviewRequested(reviewer: reviewer.name)
            case "PullRequestCommit":
                guard let commit else { return nil }
                let oid = String((commit.abbreviatedOid ?? "").filter(\.isHexDigit).prefix(12))
                let headline = String(PullRequestAnswer.clean(commit.messageHeadline ?? "").prefix(256))
                let committer = commit.author?.user?.login ?? commit.author?.name
                return .init(id: id ?? "commit-\(oid)", actor: committer.map(PullRequestAnswer.clean),
                             date: RemoteText.date(commit.committedDate), kind: .commit(oid: oid, headline: headline))
            case "HeadRefForcePushedEvent":
                kind = .forcePushed(oid: afterCommit?.abbreviatedOid.map { String($0.filter(\.isHexDigit).prefix(12)) })
            case "ReadyForReviewEvent": kind = .readyForReview
            case "ClosedEvent": kind = .closed
            case "ReopenedEvent": kind = .reopened
            case "MergedEvent": kind = .merged
            default: return nil
            }
            return .init(id: id ?? UUID().uuidString, actor: who, date: when, kind: kind)
        }
    }

    /// Remote text as the screens show it.
    enum RemoteText {
        /// Markdown kept as written, its lines intact, without other
        /// control or direction characters, its HTML as Markdown
        /// (`CommentHTML`), and cut at `limit` characters with a note.
        static func body(_ text: String, limit: Int) -> String {
            // Converted first, bounded so the work stays small however long
            // the body, then filtered, so no entity can decode into a
            // control or direction character after the filter ran.
            let converted = CommentHTML.markdown(String(text.prefix(limit * 2 + 1024)))
            let kept = String(String.UnicodeScalarView(converted.unicodeScalars.filter { scalar in
                if scalar == "\n" || scalar == "\t" { return true }
                return scalar.properties.generalCategory != .control
                    && !(0x202A...0x202E).contains(scalar.value) && !(0x2066...0x2069).contains(scalar.value)
            }))
            .trimmingCharacters(in: .whitespacesAndNewlines)
            guard kept.count > limit else { return kept }
            return String(kept.prefix(limit)) + "\n\n…"
        }

        static func date(_ text: String?) -> Date? {
            text.flatMap { try? Date($0, strategy: .iso8601) }
        }
    }
}
