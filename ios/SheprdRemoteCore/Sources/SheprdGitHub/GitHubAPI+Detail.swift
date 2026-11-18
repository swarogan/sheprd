import Foundation

extension GitHubAPI {
    /// A pull request's detail: one GraphQL query.
    public func pullRequestDetail(
        host: GitHubHost,
        owner: String,
        repo: String,
        number: UInt64,
        token: GitHubToken
    ) async throws -> PullRequestDetail {
        var request = GitHubHTTP.request(host.graphQL, token: token, method: "POST")
        request.httpBody = try PullRequestDetailQuery(owner: owner, repo: repo, number: number).body
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        let response = try await send(request, host: host, limit: .whole(max: Self.graphQLLimit))
        guard response.status == 200 else { throw GitHubHTTP.failure(response, now: now()) }
        do {
            return try PullRequestDetailAnswer.parse(response, now: now())
        } catch {
            if case let .rateLimited(until) = error { paused[host] = until }
            throw error
        }
    }

    /// The files a pull request changes, page by page up to
    /// `PullRequestFiles.pageLimit`, each page sent with the ETag of the
    /// answer last seen for it.
    public func pullRequestFiles(
        host: GitHubHost,
        owner: String,
        repo: String,
        number: UInt64,
        token: GitHubToken
    ) async throws -> PullRequestFiles {
        var files: [PullRequestFile] = []
        var budget = PullRequestFiles.patchBudget
        for page in 1...PullRequestFiles.pageLimit {
            let path = PullRequestFiles.path(owner: owner, repo: repo, number: number, page: page)
            let body = try await restPage(path, host: host, token: token)
            let read = try PullRequestFiles.parse(page: body, budget: &budget)
            files += read
            guard read.count >= PullRequestFiles.perPage else { return PullRequestFiles(files: files, truncated: false) }
        }
        return PullRequestFiles(files: files, truncated: true)
    }

    /// `rest`, for an answer up to `PullRequestFiles.pageBytes`.
    private func restPage(_ path: String, host: GitHubHost, token: GitHubToken) async throws -> Data {
        guard let url = URL(string: host.restBase.absoluteString + path) else { throw GitHubError.malformed }
        var request = GitHubHTTP.request(url, token: token)
        let key = ETagCache.Key(url: url, token: token)
        let cached = etags[key]
        if let cached { request.setValue(cached.etag, forHTTPHeaderField: "If-None-Match") }
        let response = try await send(request, host: host, limit: .whole(max: PullRequestFiles.pageBytes))
        if response.status == 304, let cached { return cached.body }
        guard (200..<300).contains(response.status) else { throw GitHubHTTP.failure(response, now: now()) }
        // Only pages of the usual size are kept, so the cache stays small.
        if let etag = response.header("etag"), response.body.count <= Self.restLimit {
            etags[key] = .init(etag: etag, body: response.body)
        }
        return response.body
    }
}
