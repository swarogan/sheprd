import Foundation
import SheprdRemoteCore

/// A GitHub access token. Its text never reaches a description, a debug
/// dump, or a log; only the `Authorization` header reads it.
public struct GitHubToken: Codable, Hashable, Sendable, CustomStringConvertible, CustomDebugStringConvertible, CustomReflectable {
    let secret: String

    public init(_ secret: String) {
        self.secret = secret
    }

    public init(from decoder: Decoder) throws {
        secret = try decoder.singleValueContainer().decode(String.self)
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        try container.encode(secret)
    }

    var authorization: String { "Bearer \(secret)" }
    var isEmpty: Bool { secret.isEmpty }

    public var description: String { "GitHubToken(redacted)" }
    public var debugDescription: String { description }
    public var customMirror: Mirror { Mirror(self, children: []) }
}

/// Where a computer's token came from.
public enum LeaseSource: String, Codable, Hashable, Sendable {
    /// Sheprd's own sign-in: an access token that expires within hours,
    /// renewed by Sheprd alone.
    case sheprd
    /// `gh auth token`, which does not expire.
    case gh
    /// A source this version does not know yet.
    case other

    public init(from decoder: Decoder) throws {
        self = Self(rawValue: try decoder.singleValueContainer().decode(String.self)) ?? .other
    }
}

/// `GET /api/v2/github/token?host=HOST` as a computer answers it.
public struct LeaseAnswer: Decodable, Sendable {
    public var host: String
    public var login: String
    public var token: GitHubToken
    /// Unix seconds; absent for a token that does not expire.
    public var expiresAt: Int64?
    public var source: LeaseSource

    public init(host: String, login: String, token: GitHubToken, expiresAt: Int64? = nil, source: LeaseSource) {
        self.host = host
        self.login = login
        self.token = token
        self.expiresAt = expiresAt
        self.source = source
    }

    enum CodingKeys: String, CodingKey {
        case host, login, token, source
        case expiresAt = "expires_at"
    }
}

/// What came of asking one computer for a token: its lease, or the HTTP
/// status it refused with (404 no account, 403 sharing off, 429 rate
/// limited, 504 Sheprd did not answer). A computer that could not be
/// reached throws instead.
public enum LeaseFetch: Sendable {
    case lease(LeaseAnswer)
    case status(Int)
}

/// Asks computer `device` for its token for `host`.
public typealias FetchLease = @Sendable (_ device: String, _ host: GitHubHost) async throws -> LeaseFetch

/// A token this phone holds for one GitHub host, and the computer that
/// lent it.
public struct GitHubLease: Codable, Hashable, Sendable {
    public var host: GitHubHost
    public var login: String
    public var token: GitHubToken
    public var expiresAt: Date?
    public var source: LeaseSource
    /// The paired computer's device id.
    public var lender: String
    /// Its name, which Settings shows.
    public var lenderName: String
    public var received: Date

    public init(host: GitHubHost, login: String, token: GitHubToken, expiresAt: Date?, source: LeaseSource, lender: String, lenderName: String, received: Date) {
        self.host = host
        self.login = login
        self.token = token
        self.expiresAt = expiresAt
        self.source = source
        self.lender = lender
        self.lenderName = lenderName
        self.received = received
    }

    /// How long before its expiry a lease is asked for again, so a read
    /// never starts with a token about to stop working.
    public static let renewal: TimeInterval = 5 * 60

    /// Whether to use it at `now` without asking again.
    public func fresh(at now: Date) -> Bool {
        expiresAt.map { $0.timeIntervalSince(now) > Self.renewal } ?? true
    }

    /// Whether GitHub may still take it at `now`.
    public func valid(at now: Date) -> Bool {
        expiresAt.map { $0 > now } ?? true
    }
}

/// A paired computer as a lease's choice of lender sees it.
public struct GitHubLender: Hashable, Sendable {
    public var device: String
    public var name: String
    /// The hosts its latest `home` lends; nil while none has arrived.
    public var hosts: Set<GitHubHost>?
    /// When it last answered; the most recent is asked first.
    public var reached: Date?

    public init(device: String, name: String, hosts: Set<GitHubHost>?, reached: Date?) {
        self.device = device
        self.name = name
        self.hosts = hosts
        self.reached = reached
    }

    /// Reads the hosts a computer's `home` lends: none from an older one.
    public init(device: String, name: String, home: Home?, reached: Date?) {
        let hosts = home.map { Set(($0.githubHosts ?? []).compactMap { GitHubHost($0.host) }) }
        self.init(device: device, name: name, hosts: hosts, reached: reached)
    }

    /// Those of `lenders` that lend `host`, the most recently reached
    /// first.
    public static func order(_ lenders: [GitHubLender], for host: GitHubHost) -> [GitHubLender] {
        lenders.filter { $0.hosts?.contains(host) == true }.sorted { a, b in
            switch (a.reached, b.reached) {
            case let (x?, y?) where x != y: x > y
            case (_?, nil): true
            case (nil, _?): false
            default: a.name < b.name
            }
        }
    }
}
