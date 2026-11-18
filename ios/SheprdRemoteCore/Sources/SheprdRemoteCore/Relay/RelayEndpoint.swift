import Foundation

/// A relay's address, as `sheprd-relay`'s `Endpoint` reads it: https
/// implied, `wss` the same as `https`, plain http only for this computer,
/// a private address, or `.local`. `RelayTests` checks both agree on every
/// form against the Rust crate's vectors.
public struct RelayEndpoint: Hashable, Codable, Sendable, CustomStringConvertible {
    public let secure: Bool
    public let host: String
    public let port: Int?
    /// The path prefix, without its trailing slash; empty for none.
    public let prefix: String

    public init?(_ text: String) {
        var text = text.trimmingCharacters(in: .whitespaces)
        if !text.contains("://") { text = "https://" + text }
        guard let components = URLComponents(string: text),
              let scheme = components.scheme?.lowercased(),
              let host = components.host?.lowercased(), !host.isEmpty,
              components.user == nil, components.password == nil,
              components.query == nil, components.fragment == nil
        else { return nil }
        switch scheme {
        case "https", "wss": secure = true
        case "http", "ws": secure = false
        default: return nil
        }
        let defaultPort = secure ? 443 : 80
        port = components.port.flatMap { $0 == defaultPort ? nil : $0 }
        self.host = host
        var path = components.percentEncodedPath
        while path.hasSuffix("/") { path.removeLast() }
        prefix = path
        guard secure || Self.local(host) else { return nil }
    }

    static func local(_ host: String) -> Bool {
        let name = host.trimmingCharacters(in: CharacterSet(charactersIn: "[]."))
        if name == "localhost" || name.hasSuffix(".localhost") || name.hasSuffix(".local") { return true }
        if name.hasPrefix("127.") || name.hasPrefix("10.") || name.hasPrefix("192.168.") || name == "::1" { return true }
        if name.hasPrefix("172."), let second = Int(name.split(separator: ".").dropFirst().first ?? ""), (16...31).contains(second) { return true }
        return name.hasPrefix("fd") || name.hasPrefix("fe80:")
    }

    /// `host[:port]` as a `Host` header carries it.
    var authority: String {
        let shown = host.contains(":") ? "[\(host)]" : host
        return port.map { "\(shown):\($0)" } ?? shown
    }

    /// The name WebSocket proofs sign over: host, port, and prefix.
    public var proofName: String { authority + prefix }

    public var description: String {
        "\(secure ? "https" : "http")://\(authority)\(prefix)"
    }

    /// Where the relay says what it is: `GET /v1/info`.
    public var info: URL? {
        URL(string: "\(description)/v1/info")
    }

    /// The WebSocket that dials `route`.
    public func dial(route: Data) -> URL? {
        URL(string: "\(secure ? "wss" : "ws")://\(authority)\(prefix)/v1/dial/\(Base64URL.encode(route))")
    }

    public init(from decoder: Decoder) throws {
        let text = try decoder.singleValueContainer().decode(String.self)
        guard let endpoint = RelayEndpoint(text) else {
            throw DecodingError.dataCorrupted(.init(codingPath: decoder.codingPath, debugDescription: "not a relay address"))
        }
        self = endpoint
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        try container.encode(description)
    }
}
