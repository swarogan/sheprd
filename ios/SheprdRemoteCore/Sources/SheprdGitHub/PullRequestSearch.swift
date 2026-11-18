import Foundation

/// The open pull requests the signed-in account wrote, and those asking for
/// its review, on any repository it can read: two GitHub searches in one
/// GraphQL query, each bounded to `limit` results. The searches travel as
/// variables, never inside the query text.
enum PullRequestSearchQuery {
    /// Pull requests read per list.
    static let limit = 50

    static let mine = "is:pr is:open author:@me archived:false sort:updated-desc"
    static let reviewRequested = "is:pr is:open review-requested:@me archived:false sort:updated-desc"

    /// The checks are counted by state rather than listed, so fifty pull
    /// requests stay a small answer.
    static let text = """
    query($mine: String!, $review: String!) {
      mine: search(type: ISSUE, query: $mine, first: \(limit)) { issueCount nodes { ...SearchedPullRequest } }
      review: search(type: ISSUE, query: $review, first: \(limit)) { issueCount nodes { ...SearchedPullRequest } }
    }
    fragment SearchedPullRequest on PullRequest {
      number title state isDraft reviewDecision updatedAt headRefName
      author { login }
      headRepositoryOwner { login }
      repository { name owner { login } }
      commits(last: 1) { nodes { commit { statusCheckRollup { contexts(first: 1) {
        checkRunCountsByState { state count }
        statusContextCountsByState { state count }
      } } } } }
    }
    """

    /// The JSON body of the request.
    static var body: Data {
        get throws {
            try JSONSerialization.data(
                withJSONObject: ["query": text, "variables": ["mine": mine, "review": reviewRequested]],
                options: [.sortedKeys]
            )
        }
    }
}

/// A pull request as a search lists it: where it is, what it is called,
/// where it stands, and its checks counted.
public struct GitHubSearchPull: Hashable, Sendable, Identifiable, Codable {
    public var host: GitHubHost
    public var owner: String
    public var repo: String
    public var number: UInt64
    public var title: String
    public var state: GitHubPullRequest.State
    public var isDraft: Bool
    public var reviewDecision: GitHubPullRequest.ReviewDecision
    public var author: String?
    public var updatedAt: Date?
    /// The branch its head is.
    public var headRefName: String
    /// Who owns its head: the repository's owner, or a fork's; nil when the
    /// fork was deleted.
    public var headOwner: String?
    public var checks: CheckCounts

    public init(
        host: GitHubHost, owner: String, repo: String, number: UInt64, title: String,
        state: GitHubPullRequest.State = .open, isDraft: Bool = false, reviewDecision: GitHubPullRequest.ReviewDecision = .none,
        author: String? = nil, updatedAt: Date? = nil, headRefName: String, headOwner: String? = nil, checks: CheckCounts = CheckCounts()
    ) {
        self.host = host
        self.owner = owner
        self.repo = repo
        self.number = number
        self.title = title
        self.state = state
        self.isDraft = isDraft
        self.reviewDecision = reviewDecision
        self.author = author
        self.updatedAt = updatedAt
        self.headRefName = headRefName
        self.headOwner = headOwner
        self.checks = checks
    }

    public var id: String { "\(host.name)/\(owner)/\(repo)#\(number)".lowercased() }

    /// `owner/repo`, as the row shows it.
    public var repository: String { "\(owner)/\(repo)" }

    /// Its page on GitHub, built from the checked parts rather than taken
    /// from the answer, so a link is always `https` on its own host.
    public var url: URL? { URL(string: "\(host.web)/\(owner)/\(repo)/pull/\(number)") }

    /// The branch a workspace would be on to carry it.
    public var branch: GitHubBranch {
        GitHubBranch(host: host, owner: owner, repo: repo, headOwner: headOwner ?? owner, branch: headRefName)
    }

    /// Checks by where they stand; skipped and neutral ones are not counted.
    public struct CheckCounts: Hashable, Sendable, Codable {
        public var failing: Int
        public var pending: Int
        public var passed: Int

        public init(failing: Int = 0, pending: Int = 0, passed: Int = 0) {
            self.failing = failing
            self.pending = pending
            self.passed = passed
        }
    }
}

/// The two lists, and how many each search found in all.
/// Codable for the phone's cache, which shows the lists last read at once.
public struct PullRequestLists: Hashable, Sendable, Codable {
    public var mine: [GitHubSearchPull]
    public var reviewRequested: [GitHubSearchPull]
    public var mineTotal: Int
    public var reviewRequestedTotal: Int

    public init(mine: [GitHubSearchPull] = [], reviewRequested: [GitHubSearchPull] = [], mineTotal: Int? = nil, reviewRequestedTotal: Int? = nil) {
        self.mine = mine
        self.reviewRequested = reviewRequested
        self.mineTotal = mineTotal ?? mine.count
        self.reviewRequestedTotal = reviewRequestedTotal ?? reviewRequested.count
    }

    public var isEmpty: Bool { mine.isEmpty && reviewRequested.isEmpty }

    /// The pull requests whose title, repository, number, author, or branch
    /// holds every word of `query`, whatever their case; all of them for an
    /// empty query. Totals become what matched, as a search finds no more.
    public func matching(_ query: String) -> PullRequestLists {
        let words = query.split(whereSeparator: \.isWhitespace).map { $0.trimmingCharacters(in: CharacterSet(charactersIn: "#")) }
            .filter { !$0.isEmpty }
        guard !words.isEmpty else { return self }
        let matches = { (pull: GitHubSearchPull) in
            let fields = [pull.title, pull.repository, "\(pull.number)", pull.author ?? "", pull.headRefName]
            return words.allSatisfy { word in fields.contains { $0.localizedCaseInsensitiveContains(word) } }
        }
        return PullRequestLists(mine: mine.filter(matches), reviewRequested: reviewRequested.filter(matches))
    }
}

/// Reading a `PullRequestSearchQuery`'s answer. Everything in it is remote
/// text: names must look like GitHub's, titles lose control and direction
/// characters, and nothing past the bounds is kept.
enum PullRequestSearchAnswer {
    static func parse(_ response: GitHubResponse, host: GitHubHost, now: Date) throws(GitHubError) -> PullRequestLists {
        guard let answer = try? JSONDecoder().decode(Answer.self, from: response.body) else { throw .malformed }
        guard let data = answer.data, let mine = data.mine, let review = data.review else {
            throw PullRequestAnswer.failure(answer.errors ?? [], response: response, now: now)
        }
        return PullRequestLists(
            mine: pulls(mine, host: host), reviewRequested: pulls(review, host: host),
            mineTotal: max(mine.issueCount ?? 0, 0), reviewRequestedTotal: max(review.issueCount ?? 0, 0)
        )
    }

    private static func pulls(_ search: Search, host: GitHubHost) -> [GitHubSearchPull] {
        var seen = Set<String>()
        return search.nodes.prefix(PullRequestSearchQuery.limit).compactMap { $0?.pull(host: host) }.filter { seen.insert($0.id).inserted }
    }

    /// A GitHub login or repository name: letters, digits, `-`, `_`, `.`.
    static func name(_ text: String?) -> String? {
        guard let text, !text.isEmpty, text.utf8.count <= 100, text != ".", text != "..",
              text.unicodeScalars.allSatisfy({ $0.isASCII && (CharacterSet.alphanumerics.contains($0) || "-_.".unicodeScalars.contains($0)) })
        else { return nil }
        return text
    }

    /// A branch name as Git allows it, bounded.
    static func branch(_ text: String?) -> String? {
        guard let text, !text.isEmpty, text.utf8.count <= 255,
              !text.unicodeScalars.contains(where: { $0.properties.generalCategory == .control || $0 == " " })
        else { return nil }
        return text
    }

    /// Counts by state, as `PullRequestAnswer.Check.state` reads one check.
    static func counts(_ contexts: Node.Contexts?) -> GitHubSearchPull.CheckCounts {
        var counts = GitHubSearchPull.CheckCounts()
        for entry in (contexts?.checkRunCountsByState ?? []) + (contexts?.statusContextCountsByState ?? []) {
            let count = min(max(entry.count, 0), 10_000)
            switch entry.state {
            case "SUCCESS": counts.passed += count
            case "FAILURE", "ERROR", "CANCELLED", "TIMED_OUT", "ACTION_REQUIRED", "STARTUP_FAILURE", "STALE": counts.failing += count
            case "NEUTRAL", "SKIPPED", "COMPLETED": break
            default: counts.pending += count
            }
        }
        return counts
    }

    struct Answer: Decodable {
        var data: DataField?
        var errors: [PullRequestAnswer.Answer.Failure]?

        struct DataField: Decodable {
            var mine: Search?
            var review: Search?
        }
    }

    struct Search: Decodable {
        var issueCount: Int?
        var nodes: [Node?]
    }

    struct Login: Decodable {
        var login: String
    }

    /// A search result: a pull request, or another kind of item, whose
    /// fields are all absent.
    struct Node: Decodable {
        var number: UInt64?
        var title: String?
        var state: String?
        var isDraft: Bool?
        var reviewDecision: String?
        var updatedAt: String?
        var headRefName: String?
        var author: Login?
        var headRepositoryOwner: Login?
        var repository: Repository?
        var commits: Commits?

        struct Repository: Decodable {
            var name: String
            var owner: Login
        }

        struct Commits: Decodable {
            var nodes: [CommitNode?]?
        }

        struct CommitNode: Decodable {
            var commit: Commit
        }

        struct Commit: Decodable {
            var statusCheckRollup: Rollup?
        }

        struct Rollup: Decodable {
            var contexts: Contexts?
        }

        struct Contexts: Decodable {
            var checkRunCountsByState: [Count]?
            var statusContextCountsByState: [Count]?
        }

        struct Count: Decodable {
            var state: String
            var count: Int
        }

        func pull(host: GitHubHost) -> GitHubSearchPull? {
            guard let number, number > 0, let title,
                  let owner = PullRequestSearchAnswer.name(repository?.owner.login),
                  let repo = PullRequestSearchAnswer.name(repository?.name),
                  let head = PullRequestSearchAnswer.branch(headRefName)
            else { return nil }
            return GitHubSearchPull(
                host: host, owner: owner, repo: repo, number: number,
                title: String(PullRequestAnswer.clean(title).prefix(512)),
                state: state.flatMap(GitHubPullRequest.State.init(rawValue:)) ?? .unknown,
                isDraft: isDraft ?? false,
                reviewDecision: reviewDecision.flatMap(GitHubPullRequest.ReviewDecision.init(rawValue:)) ?? .none,
                author: PullRequestSearchAnswer.name(author?.login),
                updatedAt: updatedAt.flatMap { try? Date($0, strategy: .iso8601) },
                headRefName: head,
                headOwner: PullRequestSearchAnswer.name(headRepositoryOwner?.login),
                checks: PullRequestSearchAnswer.counts(commits?.nodes?.first??.commit.statusCheckRollup?.contexts)
            )
        }
    }
}

extension GitHubAPI {
    /// The account's open pull requests and those asking for its review on
    /// `host`, in one query.
    func searchPullRequests(host: GitHubHost, token: GitHubToken) async throws -> PullRequestLists {
        var request = GitHubHTTP.request(host.graphQL, token: token, method: "POST")
        request.httpBody = try PullRequestSearchQuery.body
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        let response = try await send(request, host: host, limit: .whole(max: Self.graphQLLimit))
        guard response.status == 200 else { throw GitHubHTTP.failure(response, now: now()) }
        do {
            return try PullRequestSearchAnswer.parse(response, host: host, now: now())
        } catch {
            // GraphQL says to wait with a 200, as the branch reads hear it.
            if case let .rateLimited(until) = error { paused[host] = until }
            throw error
        }
    }
}
