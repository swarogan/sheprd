import Foundation
import Testing
@testable import SheprdGitHub
@testable import SheprdRemoteCore

struct GitHubAPITests {
    private let host = GitHubFixtures.host
    private let token = GitHubToken("ghu_secret")

    private func read(_ api: GitHubAPI) async throws -> [String: Result<GitHubPullRequest?, GitHubError>] {
        try await api.pullRequests(host: host, owner: "acme", repo: "app", branches: ["feature"], headOwner: { _ in "acme" }, token: token)
    }

    @Test func aQueryGoesToTheHostsGraphQLWithTheToken() async throws {
        let transport = FakeTransport([try GitHubFixtures.response("open_pr")])
        let api = GitHubAPI(transport: transport.transport, now: { GitHubFixtures.start })
        let found = try await read(api)
        #expect(try found["feature"]?.get()?.number == 42)
        let request = try #require(transport.requests.first)
        #expect(request.url?.absoluteString == "https://api.github.com/graphql")
        #expect(request.httpMethod == "POST")
        #expect(request.value(forHTTPHeaderField: "Authorization") == "Bearer ghu_secret")
        #expect(request.value(forHTTPHeaderField: "X-GitHub-Api-Version") == "2022-11-28")
    }

    @Test func anEnterpriseServerIsAskedAtItsOwnAddress() async throws {
        let transport = FakeTransport([try GitHubFixtures.response("no_pr")])
        let api = GitHubAPI(transport: transport.transport)
        let enterprise = try #require(GitHubHost("github.example.com"))
        _ = try await api.pullRequests(host: enterprise, owner: "acme", repo: "app", branches: ["feature"], headOwner: { _ in "acme" }, token: token)
        #expect(transport.requests.first?.url?.absoluteString == "https://github.example.com/api/graphql")
    }

    @Test func aRefusedTokenIsUnauthorized() async throws {
        let api = GitHubAPI(transport: FakeTransport([GitHubResponse(status: 401)]).transport)
        await #expect(throws: GitHubError.unauthorized) { try await read(api) }
    }

    @Test func singleSignOnAndTheRateLimitAreTold403sApart() async throws {
        let sso = GitHubResponse(status: 403, headers: ["X-GitHub-SSO": "required; url=https://github.com/orgs/acme/sso"])
        let api = GitHubAPI(transport: FakeTransport([sso]).transport, now: { GitHubFixtures.start })
        await #expect(throws: GitHubError.ssoRequired) { try await read(api) }

        let reset = GitHubFixtures.start.addingTimeInterval(900)
        let spent = GitHubResponse(status: 403, headers: ["X-RateLimit-Remaining": "0", "X-RateLimit-Reset": "\(Int(reset.timeIntervalSince1970))"])
        let transport = FakeTransport([spent])
        let limited = GitHubAPI(transport: transport.transport, now: { GitHubFixtures.start })
        await #expect(throws: GitHubError.rateLimited(until: reset)) { try await read(limited) }
        // Nothing is sent again before the reset.
        await #expect(throws: GitHubError.rateLimited(until: reset)) { try await read(limited) }
        #expect(transport.requests.count == 1)

        let secondary = GitHubResponse(status: 403, headers: ["Retry-After": "120"], body: Data(#"{"message":"You have exceeded a secondary rate limit."}"#.utf8))
        let paused = GitHubAPI(transport: FakeTransport([secondary]).transport, now: { GitHubFixtures.start })
        await #expect(throws: GitHubError.rateLimited(until: GitHubFixtures.start.addingTimeInterval(120))) { try await read(paused) }

        let plain = GitHubAPI(transport: FakeTransport([GitHubResponse(status: 403, body: Data(#"{"message":"Resource not accessible by integration"}"#.utf8))]).transport)
        await #expect(throws: GitHubError.forbidden) { try await read(plain) }
    }

    @Test func aMissingRepositoryIsNotFound() async throws {
        let api = GitHubAPI(transport: FakeTransport([GitHubResponse(status: 404)]).transport)
        await #expect(throws: GitHubError.notFound) { try await read(api) }
        let graphQL = GitHubAPI(transport: FakeTransport([try GitHubFixtures.response("not_found")]).transport)
        await #expect(throws: GitHubError.notFound) { try await read(graphQL) }
    }

    @Test func anUnchangedRESTAnswerIsReusedThroughItsETag() async throws {
        let transport = FakeTransport([
            GitHubResponse(status: 200, headers: ["ETag": #"W/"abc""#], body: Data(#"{"a":1}"#.utf8)),
            GitHubResponse(status: 304),
        ])
        let api = GitHubAPI(transport: transport.transport)
        let first = try await api.rest("/repos/acme/app", host: host, token: token)
        let second = try await api.rest("/repos/acme/app", host: host, token: token)
        #expect(first == second)
        #expect(transport.requests[0].value(forHTTPHeaderField: "If-None-Match") == nil)
        #expect(transport.requests[1].value(forHTTPHeaderField: "If-None-Match") == #"W/"abc""#)
        #expect(transport.requests[1].url?.absoluteString == "https://api.github.com/repos/acme/app")
    }

    @Test func aJobLogFollowsTheRedirectWithoutTheToken() async throws {
        let log = "2026-01-02T03:04:05.1234567Z first\n2026-01-02T03:04:06.1234567Z ##[error]boom\nPost job cleanup.\n"
        let transport = FakeTransport([
            GitHubResponse(status: 302, headers: ["Location": "https://storage.example/log?sig=1"]),
            GitHubResponse(status: 200, body: Data(log.utf8)),
        ])
        let api = GitHubAPI(transport: transport.transport)
        let excerpt = try await api.jobLog(host: host, owner: "acme", repo: "app", job: 70, token: token)
        #expect(excerpt == "first\n##[error]boom")
        #expect(transport.requests[0].url?.absoluteString == "https://api.github.com/repos/acme/app/actions/jobs/70/logs")
        #expect(transport.requests[0].value(forHTTPHeaderField: "Authorization") == "Bearer ghu_secret")
        #expect(transport.requests[1].url?.absoluteString == "https://storage.example/log?sig=1")
        #expect(transport.requests[1].value(forHTTPHeaderField: "Authorization") == nil)
    }

    @Test func aJobLogRedirectToAPlainAddressIsRefused() async throws {
        let transport = FakeTransport([GitHubResponse(status: 302, headers: ["Location": "http://storage.example/log"])])
        let api = GitHubAPI(transport: transport.transport)
        await #expect(throws: GitHubError.location) {
            try await api.jobLog(host: host, owner: "acme", repo: "app", job: 70, token: token)
        }
        #expect(transport.requests.count == 1)
    }

    @Test func aBodyPastItsLimitIsRefused() async throws {
        let bytes = AsyncStream<UInt8> { continuation in
            for byte in Array("0123456789".utf8) { continuation.yield(byte) }
            continuation.finish()
        }
        await #expect(throws: GitHubError.tooLarge) { try await GitHubHTTP.read(bytes, limit: .whole(max: 9), expected: -1) }
        let again = AsyncStream<UInt8> { continuation in
            for byte in Array("0123456789".utf8) { continuation.yield(byte) }
            continuation.finish()
        }
        #expect(try await GitHubHTTP.read(again, limit: .tail(keep: 3, max: 10), expected: -1) == Data("789".utf8))
    }
}
