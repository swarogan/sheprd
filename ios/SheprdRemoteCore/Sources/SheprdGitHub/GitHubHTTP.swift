import Foundation

/// An answer from GitHub, its headers' names in lowercase.
public struct GitHubResponse: Sendable {
    public var status: Int
    public var headers: [String: String]
    public var body: Data

    public init(status: Int, headers: [String: String] = [:], body: Data = Data()) {
        self.status = status
        self.headers = Dictionary(headers.map { ($0.key.lowercased(), $0.value) }) { _, last in last }
        self.body = body
    }

    public func header(_ name: String) -> String? {
        headers[name.lowercased()]
    }
}

/// How much of an answer's body is read.
public enum BodyLimit: Sendable, Equatable {
    /// All of it, refused past `max` bytes.
    case whole(max: Int)
    /// Its last `keep` bytes, refused past `max` bytes read (`LogTail`).
    case tail(keep: Int, max: Int)
}

/// Sends one request and answers without following a redirect, so a token
/// never travels to wherever one points. Injected, so tests answer from
/// fixtures.
public typealias GitHubTransport = @Sendable (URLRequest, BodyLimit) async throws -> GitHubResponse

/// Why a direct read of GitHub failed. Each one sends the read to the
/// computer instead.
public enum GitHubError: Error, Equatable, Sendable {
    /// The token was refused (401): renewed by its computer, or revoked.
    case unauthorized
    /// The organization requires SAML single sign-on that this token has
    /// not been authorized for (403 with `X-GitHub-SSO`).
    case ssoRequired
    /// GitHub's rate limit, primary or secondary: nothing is sent to this
    /// host before `until`.
    case rateLimited(until: Date)
    /// The token may not read this (403 otherwise).
    case forbidden
    /// No such repository, job, or log, or one the token cannot see (404).
    case notFound
    case status(Int)
    /// An answer this phone cannot read.
    case malformed
    /// An answer longer than allowed.
    case tooLarge
    /// A log's redirect that does not lead to an `https` address.
    case location
    /// More than one open pull request on the branch.
    case ambiguous
}

public enum GitHubHTTP {
    static let apiVersion = "2022-11-28"

    /// A request with the headers GitHub asks API clients to send.
    static func request(_ url: URL, token: GitHubToken?, method: String = "GET", timeout: TimeInterval = 20) -> URLRequest {
        var request = URLRequest(url: url, cachePolicy: .reloadIgnoringLocalCacheData, timeoutInterval: timeout)
        request.httpMethod = method
        request.setValue("Sheprd", forHTTPHeaderField: "User-Agent")
        request.setValue("application/vnd.github+json", forHTTPHeaderField: "Accept")
        request.setValue(apiVersion, forHTTPHeaderField: "X-GitHub-Api-Version")
        if let token { request.setValue(token.authorization, forHTTPHeaderField: "Authorization") }
        return request
    }

    /// What a refusal means. A 403 or 429 that names a wait, or says the
    /// quota is spent, is the rate limit; a 403 with an SSO challenge is
    /// single sign-on.
    static func failure(_ response: GitHubResponse, now: Date) -> GitHubError {
        switch response.status {
        case 401: return .unauthorized
        case 404, 410: return .notFound
        case 403, 429:
            if let until = cooldown(response, now: now) { return .rateLimited(until: until) }
            if response.header("x-github-sso") != nil { return .ssoRequired }
            return response.status == 429 ? .rateLimited(until: now.addingTimeInterval(defaultWait)) : .forbidden
        default: return .status(response.status)
        }
    }

    /// Waited when GitHub names no time.
    static let defaultWait: TimeInterval = 3600
    static let shortestWait: TimeInterval = 60
    static let longestWait: TimeInterval = 86400

    /// Until when the rate limit holds, from `Retry-After` and
    /// `X-RateLimit-Reset`, the later of the two; nil when the answer is
    /// not about the rate limit.
    static func cooldown(_ response: GitHubResponse, now: Date) -> Date? {
        let number = { (name: String) in response.header(name).flatMap { Double($0.trimmingCharacters(in: .whitespaces)) } }
        let retry = number("retry-after")
        let spent = number("x-ratelimit-remaining") == 0
        let secondary = String(decoding: response.body.prefix(4096), as: UTF8.self).lowercased().contains("rate limit")
        guard retry != nil || spent || secondary else { return nil }
        let reset = spent ? number("x-ratelimit-reset").map { $0 - now.timeIntervalSince1970 } : nil
        let wait = [retry, reset].compactMap { $0 }.max() ?? defaultWait
        return now.addingTimeInterval(min(max(wait, shortestWait), longestWait))
    }

    /// The transport the app uses: a session of its own trusting the
    /// system's roots, ephemeral, and never following a redirect.
    public static func urlSession() -> GitHubTransport {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.timeoutIntervalForRequest = 20
        configuration.waitsForConnectivity = false
        configuration.urlCache = nil
        let session = URLSession(configuration: configuration, delegate: NoRedirects(), delegateQueue: nil)
        return { request, limit in
            // Every address is GitHub's own or its storage's, all `https`.
            guard request.url?.scheme?.lowercased() == "https" else { throw GitHubError.location }
            let (bytes, response) = try await session.bytes(for: request)
            defer { bytes.task.cancel() }
            guard let http = response as? HTTPURLResponse else { throw GitHubError.malformed }
            var headers: [String: String] = [:]
            for (name, value) in http.allHeaderFields {
                if let name = name as? String, let value = value as? String { headers[name.lowercased()] = value }
            }
            let body = try await read(bytes, limit: limit, expected: http.expectedContentLength)
            return GitHubResponse(status: http.statusCode, headers: headers, body: body)
        }
    }

    static func read<Bytes: AsyncSequence>(_ bytes: Bytes, limit: BodyLimit, expected: Int64) async throws -> Data
        where Bytes.Element == UInt8
    {
        switch limit {
        case let .whole(max):
            guard expected <= Int64(max) else { throw GitHubError.tooLarge }
            var body = Data()
            if expected > 0 { body.reserveCapacity(Int(expected)) }
            for try await byte in bytes {
                guard body.count < max else { throw GitHubError.tooLarge }
                body.append(byte)
            }
            return body
        case let .tail(keep, max):
            guard expected <= Int64(max) else { throw GitHubError.tooLarge }
            var tail = LogTail(keep: keep, limit: max)
            var chunk: [UInt8] = []
            chunk.reserveCapacity(16 * 1024)
            for try await byte in bytes {
                chunk.append(byte)
                guard chunk.count == 16 * 1024 else { continue }
                guard tail.append(chunk) else { throw GitHubError.tooLarge }
                chunk.removeAll(keepingCapacity: true)
            }
            guard tail.append(chunk) else { throw GitHubError.tooLarge }
            return tail.bytes
        }
    }
}

/// Refuses every redirect, so the request's own answer, its `Location`
/// included, reaches the caller.
private final class NoRedirects: NSObject, URLSessionTaskDelegate, Sendable {
    func urlSession(
        _ session: URLSession,
        task: URLSessionTask,
        willPerformHTTPRedirection response: HTTPURLResponse,
        newRequest request: URLRequest
    ) async -> URLRequest? {
        nil
    }
}
