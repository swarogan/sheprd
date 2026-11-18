import Foundation

extension Computer {
    /// How Settings says the computer is reached: the kind of its first
    /// address a phone can use, such as Tailscale, then "Relay" when it
    /// also listens on relays, so it is reached when that address is not.
    public var reachLabel: String? {
        let kind = origins.lazy.map(OriginKind.init(origin:)).first { $0 != .loopback }
        let parts = [kind?.label, relay == nil ? nil : Self.relayLabel].compactMap { $0 }
        return parts.isEmpty ? nil : parts.joined(separator: " · ")
    }

    /// The relays the phone may reach the computer through, as their
    /// host and port.
    public var relayHosts: [String] {
        relay?.relays.map { endpoint in
            let host = endpoint.host.contains(":") ? "[\(endpoint.host)]" : endpoint.host
            return endpoint.port.map { "\(host):\($0)" } ?? host
        } ?? []
    }

    public static let relayLabel = "Relay"
}
