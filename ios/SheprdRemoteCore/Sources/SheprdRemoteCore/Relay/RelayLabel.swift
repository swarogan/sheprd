import Foundation

/// What the computer's owner calls one of its relays in `[remote] relays`
/// (`sheprd_remote_control::relay_label`): a name such as "fra3", a line
/// such as "not for public use", and tags. Each may be missing, and the
/// computer leaves out relays it has no label for.
public struct RelayLabel: Codable, Hashable, Sendable {
    public var name: String?
    public var description: String?
    public var tags: [String]

    public init(name: String? = nil, description: String? = nil, tags: [String] = []) {
        self.name = name
        self.description = description
        self.tags = tags
    }

    enum CodingKeys: String, CodingKey { case name, description, tags }

    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        name = try c.decodeIfPresent(String.self, forKey: .name)
        description = try c.decodeIfPresent(String.self, forKey: .description)
        // The computer leaves out a label's empty tags.
        tags = try c.decodeIfPresent([String].self, forKey: .tags) ?? []
    }
}

extension RelayRecord {
    /// The labels `info` gives the relays it names, keyed as `relays`
    /// writes each endpoint, so a label written another way still finds
    /// its relay; labels for relays not listed are dropped.
    public static func labels(from info: API.RelayInfo) -> [String: RelayLabel] {
        let named = Set(info.relays.compactMap(RelayEndpoint.init))
        var labels: [String: RelayLabel] = [:]
        for (text, label) in info.labels ?? [:] {
            guard let endpoint = RelayEndpoint(text), named.contains(endpoint) else { continue }
            labels[endpoint.description] = label
        }
        return labels
    }

    /// What the computer's owner calls `relay`, if anything.
    public func label(for relay: RelayEndpoint) -> RelayLabel? {
        labels[relay.description]
    }
}
