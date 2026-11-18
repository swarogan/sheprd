import Foundation
import SheprdRemoteCore

/// A branch's pull request as GitHub reports it, read directly by the
/// phone. Its checks and tone follow the computer's rules
/// (`crates/sheprd-github/src/pull_request/model.rs`), so a badge reads
/// the same whichever side read it.
public struct GitHubPullRequest: Hashable, Sendable {
    public var number: UInt64
    public var title: String
    public var url: String
    public var state: State
    public var isDraft: Bool
    public var reviewDecision: ReviewDecision
    public var mergeable: Mergeable
    public var mergeState: MergeState
    public var author: String?
    public var headRefOid: String
    public var checks: [PullRequestCheck]
    /// GitHub has more than the first 100 checks.
    public var moreChecks: Bool

    public enum State: String, Hashable, Sendable, Codable {
        case open = "OPEN", closed = "CLOSED", merged = "MERGED", unknown
    }

    public enum ReviewDecision: String, Hashable, Sendable, Codable {
        case approved = "APPROVED"
        case changesRequested = "CHANGES_REQUESTED"
        case reviewRequired = "REVIEW_REQUIRED"
        case none
    }

    public enum Mergeable: String, Hashable, Sendable {
        case mergeable = "MERGEABLE", conflicting = "CONFLICTING", unknown
    }

    public enum MergeState: String, Hashable, Sendable {
        case clean = "CLEAN", dirty = "DIRTY", behind = "BEHIND", blocked = "BLOCKED"
        case unstable = "UNSTABLE", draft = "DRAFT", hasHooks = "HAS_HOOKS", unknown
    }

    /// What its badge says, lifecycle first, then where an open, ready
    /// pull request stands.
    public var tone: Home.PullRequest.Tone {
        switch state {
        case .merged: return .merged
        case .closed: return .attention
        case .unknown: return .other
        case .open where isDraft: return .other
        case .open: break
        }
        let failed = checks.contains { $0.state == .failed }
        let running = checks.contains { $0.state == .pending }
        // GitHub also reports UNSTABLE while checks still run, so it only
        // means attention once nothing is pending.
        if mergeState == .dirty || (mergeState == .unstable && !running)
            || reviewDecision == .changesRequested || failed {
            return .attention
        }
        if running || reviewDecision == .reviewRequired { return .waiting }
        switch mergeState {
        case .clean: return .ready
        case .blocked, .behind: return .held
        default: return .waiting
        }
    }

    /// Its checks as the Changes screen lists them.
    public var checkList: PullRequestChecks {
        PullRequestChecks(number: number, checks: checks)
    }

    /// Its badge, as a computer's `home` carries one.
    public var badge: Home.PullRequest {
        Home.PullRequest(number: number, url: url, tone: tone)
    }
}

/// Reading a `PullRequestQuery`'s answer.
enum PullRequestAnswer {
    /// Each branch's pull request, none, or why it could not be told;
    /// throws when the repository itself could not be read.
    static func parse(
        _ response: GitHubResponse,
        query: PullRequestQuery,
        headOwner: (String) -> String,
        headOid: (String) -> String? = { _ in nil },
        host: GitHubHost,
        now: Date
    ) throws(GitHubError) -> [String: Result<GitHubPullRequest?, GitHubError>] {
        guard let answer = try? JSONDecoder().decode(Answer.self, from: response.body) else { throw .malformed }
        guard let repository = answer.data?.repository else {
            throw failure(answer.errors ?? [], response: response, now: now)
        }
        var found: [String: Result<GitHubPullRequest?, GitHubError>] = [:]
        for (index, branch) in query.branches.enumerated() {
            guard let nodes = repository.branches[PullRequestQuery.alias(index)] else {
                found[branch] = .failure(.malformed)
                continue
            }
            found[branch] = Result { () throws(GitHubError) in
                try pick(nodes, branch: branch, headOwner: headOwner(branch), headOid: headOid(branch), host: host)
            }
        }
        return found
    }

    /// What GraphQL's errors amount to when no repository came back.
    static func failure(_ errors: [Answer.Failure], response: GitHubResponse, now: Date) -> GitHubError {
        let types = Set(errors.compactMap(\.type))
        if types.contains("RATE_LIMITED") {
            return .rateLimited(until: GitHubHTTP.cooldown(response, now: now) ?? now.addingTimeInterval(GitHubHTTP.defaultWait))
        }
        if response.header("x-github-sso") != nil { return .ssoRequired }
        if types.contains("NOT_FOUND") { return .notFound }
        if types.contains("FORBIDDEN") { return .forbidden }
        return .malformed
    }

    /// The one pull request a branch stands for: among several, only its
    /// head owner's; then the single open one, else the most recently
    /// updated, which GitHub lists first. Two open ones stay ambiguous.
    /// The head owner's pull requests come first, so an open one there is
    /// the branch's even when an older one ends at the checkout's commit
    /// (a reused branch). Only when the owner has no open one does the
    /// checkout's commit (`headOid`) decide: a fork's pull request on the
    /// upstream repository the computer names.
    static func pick(_ nodes: [Node], branch: String, headOwner: String, headOid: String? = nil, host: GitHubHost) throws(GitHubError) -> GitHubPullRequest? {
        guard !nodes.isEmpty else { return nil }
        let oid = headOid.flatMap { $0.isEmpty ? nil : $0 }
        let byOwner = nodes.filter { $0.headRepositoryOwner?.login.caseInsensitiveCompare(headOwner) == .orderedSame }
        let byOid = oid.map { oid in nodes.filter { $0.headRefOid.caseInsensitiveCompare(oid) == .orderedSame } } ?? []
        let isOpen = { (node: Node) in node.state == "OPEN" }
        let candidates: [Node]
        if byOwner.contains(where: isOpen) {
            candidates = byOwner
        } else if byOid.contains(where: isOpen) {
            candidates = byOid
        } else if !byOwner.isEmpty {
            candidates = byOwner
        } else if !byOid.isEmpty {
            candidates = byOid
        } else {
            // Nobody's head here is the branch's.
            throw nodes.count > 1 ? .ambiguous : .malformed
        }
        let open = candidates.filter(isOpen)
        guard open.count <= 1 else { throw .ambiguous }
        guard let node = open.first ?? candidates.first else { return nil }
        guard node.number > 0, node.headRefName == branch else { throw .malformed }
        return node.pullRequest(host: host)
    }

    struct Answer: Decodable {
        var data: DataField?
        var errors: [Failure]?

        struct DataField: Decodable {
            var repository: Repository?
        }

        struct Failure: Decodable {
            var type: String?
        }
    }

    /// The repository field: `nameWithOwner` and one aliased list per
    /// branch.
    struct Repository: Decodable {
        var branches: [String: [Node]] = [:]

        struct Key: CodingKey {
            var stringValue: String
            var intValue: Int? { nil }
            init(stringValue: String) { self.stringValue = stringValue }
            init?(intValue: Int) { nil }
        }

        struct Connection: Decodable {
            var nodes: [Node?]
        }

        init(from decoder: Decoder) throws {
            let container = try decoder.container(keyedBy: Key.self)
            for key in container.allKeys where key.stringValue.hasPrefix("b") {
                // A branch GitHub could not answer for is null.
                guard let connection = try container.decodeIfPresent(Connection.self, forKey: key) else { continue }
                branches[key.stringValue] = connection.nodes.compactMap { $0 }
            }
        }
    }

    struct Login: Decodable {
        var login: String
    }

    struct Node: Decodable {
        var number: UInt64
        var title: String
        var url: String
        var state: String
        var isDraft: Bool
        var reviewDecision: String?
        var mergeable: String?
        var mergeStateStatus: String?
        var headRefName: String
        var headRefOid: String
        var author: Login?
        var headRepositoryOwner: Login?
        var commits: Commits?

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
            var contexts: Contexts
        }

        struct Contexts: Decodable {
            var pageInfo: PageInfo?
            var nodes: [Check?]
        }

        struct PageInfo: Decodable {
            var hasNextPage: Bool
        }

        func pullRequest(host: GitHubHost) -> GitHubPullRequest {
            let contexts = commits?.nodes?.first??.commit.statusCheckRollup?.contexts
            return GitHubPullRequest(
                number: number,
                title: clean(title),
                url: url,
                state: GitHubPullRequest.State(rawValue: state) ?? .unknown,
                isDraft: isDraft,
                reviewDecision: reviewDecision.flatMap(GitHubPullRequest.ReviewDecision.init(rawValue:)) ?? .none,
                mergeable: mergeable.flatMap(GitHubPullRequest.Mergeable.init(rawValue:)) ?? .unknown,
                mergeState: mergeStateStatus.flatMap(GitHubPullRequest.MergeState.init(rawValue:)) ?? .unknown,
                author: author?.login,
                headRefOid: headRefOid.count == 40 && headRefOid.allSatisfy(\.isHexDigit) ? headRefOid : "",
                checks: (contexts?.nodes ?? []).compactMap { $0?.check(host: host) },
                moreChecks: contexts?.pageInfo?.hasNextPage ?? false
            )
        }
    }

    /// One status check: a `StatusContext` reports through `state`; a
    /// `CheckRun` through `conclusion`, once its `status` says it finished.
    struct Check: Decodable {
        var __typename: String
        var name: String?
        var context: String?
        var status: String?
        var conclusion: String?
        var detailsUrl: String?
        var databaseId: UInt64?
        var state: String?
        var targetUrl: String?

        var isStatus: Bool { __typename == "StatusContext" }

        func check(host: GitHubHost) -> PullRequestCheck {
            let url = (isStatus ? targetUrl : detailsUrl).flatMap { linkable($0) ? $0 : nil }
            let outcome: String? = if isStatus { state } else if status == "COMPLETED" { conclusion } else { nil }
            // Other apps' check runs have ids too, but no log GitHub serves.
            let job = isStatus ? nil : databaseId.flatMap { id in url.map { actions($0, host: host) } == true ? id : nil }
            return PullRequestCheck(
                name: String(clean(name ?? context ?? "").prefix(128)),
                state: Self.state(outcome),
                job: job,
                url: url
            )
        }

        static func state(_ outcome: String?) -> PullRequestCheck.State {
            switch outcome {
            case "SUCCESS": .passed
            case "FAILURE", "ERROR", "CANCELLED", "TIMED_OUT", "ACTION_REQUIRED", "STARTUP_FAILURE", "STALE": .failed
            case "NEUTRAL", "SKIPPED": .skipped
            default: .pending
            }
        }
    }

    /// Whether a check's link may be offered: remote text a phone opens.
    static func linkable(_ url: String) -> Bool {
        url.utf8.count <= 2048 && url.hasPrefix("https://") && !url.unicodeScalars.contains { $0.properties.generalCategory == .control }
    }

    /// Whether `url` is a run on the host's GitHub Actions.
    static func actions(_ url: String, host: GitHubHost) -> Bool {
        url.lowercased().hasPrefix(host.web + "/") && url.contains("/actions/runs/")
    }

    /// Remote text without control or direction characters.
    static func clean(_ text: String) -> String {
        String(String.UnicodeScalarView(text.unicodeScalars.filter { scalar in
            scalar.properties.generalCategory != .control
                && !(0x202A...0x202E).contains(scalar.value) && !(0x2066...0x2069).contains(scalar.value)
        }))
    }
}
