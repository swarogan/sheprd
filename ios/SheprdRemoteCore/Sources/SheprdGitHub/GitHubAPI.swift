import Foundation

/// GitHub's APIs as the phone reads them with a leased token: GraphQL for
/// branches' pull requests, REST for a failing job's log. It keeps each
/// host's rate limit, so nothing is sent before GitHub said it may be,
/// and REST answers' ETags, so an unchanged answer costs no quota.
public actor GitHubAPI {
    private let transport: GitHubTransport
    let now: @Sendable () -> Date
    var paused: [GitHubHost: Date] = [:]
    var etags = ETagCache()

    public init(transport: @escaping GitHubTransport, now: @escaping @Sendable () -> Date = { Date() }) {
        self.transport = transport
        self.now = now
    }

    /// The app's: system trust, no redirects followed.
    static func live() -> GitHubAPI {
        GitHubAPI(transport: GitHubHTTP.urlSession())
    }

    /// GraphQL answers are a few hundred bytes per check.
    static let graphQLLimit = 2 * 1024 * 1024
    static let restLimit = 1024 * 1024

    /// Each of `branches`' pull requests in `owner/repo`: one query per
    /// `PullRequestQuery.batchLimit` branches. `headOwner` names who owns
    /// each branch's head: the repository's owner, or a fork's.
    public func pullRequests(
        host: GitHubHost,
        owner: String,
        repo: String,
        branches: [String],
        headOwner: @Sendable (String) -> String,
        headOid: @Sendable (String) -> String? = { _ in nil },
        token: GitHubToken
    ) async throws -> [String: Result<GitHubPullRequest?, GitHubError>] {
        var found: [String: Result<GitHubPullRequest?, GitHubError>] = [:]
        for query in PullRequestQuery.batches(owner: owner, repo: repo, branches: branches) {
            var request = GitHubHTTP.request(host.graphQL, token: token, method: "POST")
            request.httpBody = try query.body
            request.setValue("application/json", forHTTPHeaderField: "Content-Type")
            let response = try await send(request, host: host, limit: .whole(max: Self.graphQLLimit))
            guard response.status == 200 else { throw GitHubHTTP.failure(response, now: now()) }
            do {
                let answers = try PullRequestAnswer.parse(response, query: query, headOwner: headOwner, headOid: headOid, host: host, now: now())
                found.merge(answers) { _, last in last }
            } catch {
                if case let .rateLimited(until) = error { paused[host] = until }
                throw error
            }
        }
        return found
    }

    /// A REST path's JSON, sent with the ETag of the answer last seen for
    /// it: a 304 answers that again.
    func rest(_ path: String, host: GitHubHost, token: GitHubToken) async throws -> Data {
        guard let url = URL(string: host.restBase.absoluteString + path) else { throw GitHubError.malformed }
        var request = GitHubHTTP.request(url, token: token)
        let key = ETagCache.Key(url: url, token: token)
        let cached = etags[key]
        if let cached { request.setValue(cached.etag, forHTTPHeaderField: "If-None-Match") }
        let response = try await send(request, host: host, limit: .whole(max: Self.restLimit))
        if response.status == 304, let cached { return cached.body }
        guard (200..<300).contains(response.status) else { throw GitHubHTTP.failure(response, now: now()) }
        if let etag = response.header("etag") { etags[key] = .init(etag: etag, body: response.body) }
        return response.body
    }

    /// The excerpt of job `job`'s log in `owner/repo`. GitHub answers with
    /// a redirect to storage that needs no token, so the token goes to
    /// GitHub alone; the log's end is kept, and cut as the computer does.
    func jobLog(host: GitHubHost, owner: String, repo: String, job: UInt64, token: GitHubToken) async throws -> String {
        let path = "/repos/\(Self.segment(owner))/\(Self.segment(repo))/actions/jobs/\(job)/logs"
        guard let url = URL(string: host.restBase.absoluteString + path) else { throw GitHubError.malformed }
        let limit = BodyLimit.tail(keep: CheckLog.tail, max: CheckLog.readLimit)
        let first = try await send(GitHubHTTP.request(url, token: token), host: host, limit: limit)
        let log: GitHubResponse
        if (300..<400).contains(first.status) {
            guard let storage = CheckLog.storage(status: first.status, location: first.header("location")) else {
                throw GitHubError.location
            }
            var request = URLRequest(url: storage, cachePolicy: .reloadIgnoringLocalCacheData, timeoutInterval: 20)
            request.setValue("Sheprd", forHTTPHeaderField: "User-Agent")
            log = try await transport(request, limit)
            guard (200..<300).contains(log.status) else { throw GitHubError.status(log.status) }
        } else if (200..<300).contains(first.status) {
            log = first
        } else {
            throw GitHubHTTP.failure(first, now: now())
        }
        return CheckLog.excerpt(String(decoding: log.body, as: UTF8.self))
    }

    /// Whether GitHub asked to wait before `host` is read again.
    func isPaused(_ host: GitHubHost) -> Bool {
        paused[host].map { $0 > now() } ?? false
    }

    /// Sends `request` unless `host` is paused, and pauses it when GitHub
    /// says to wait.
    func send(_ request: URLRequest, host: GitHubHost, limit: BodyLimit) async throws -> GitHubResponse {
        let now = now()
        if let until = paused[host] {
            if until > now { throw GitHubError.rateLimited(until: until) }
            paused[host] = nil
        }
        let response = try await transport(request, limit)
        if case let .rateLimited(until) = GitHubHTTP.failure(response, now: now), response.status == 403 || response.status == 429 {
            paused[host] = until
        }
        return response
    }

    static func segment(_ text: String) -> String {
        text.addingPercentEncoding(withAllowedCharacters: .urlPathAllowed.subtracting(CharacterSet(charactersIn: "/"))) ?? text
    }
}

/// REST answers by address and token, for `If-None-Match`; bounded, the
/// oldest dropped first.
struct ETagCache {
    static let limit = 64

    struct Key: Hashable {
        var url: URL
        /// A different account may see a different answer.
        var token: GitHubToken
    }

    struct Entry {
        var etag: String
        var body: Data
    }

    private var entries: [Key: Entry] = [:]
    private var order: [Key] = []

    subscript(key: Key) -> Entry? {
        get { entries[key] }
        set {
            order.removeAll { $0 == key }
            entries[key] = newValue
            guard newValue != nil else { return }
            order.append(key)
            if order.count > Self.limit { entries[order.removeFirst()] = nil }
        }
    }
}
