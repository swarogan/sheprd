import CryptoKit
import Foundation
import SheprdTunnel

/// How this phone reaches one computer through its relays: what the
/// computer said over a path the phone already trusted (`GET /api/relay`),
/// and the keys the phone made for it and sent (`POST /api/relay`). Kept
/// in the computer's Keychain record, as its device key is.
public struct RelayRecord: Codable, Hashable, Sendable {
    /// The relays the computer listens on, nearest to it first.
    public var relays: [RelayEndpoint]
    /// The computer's round trip to each relay it listens on, in
    /// milliseconds, keyed by endpoint; nil from a daemon that does not
    /// report them.
    public var listenerRtt: [String: Int]?
    /// The computer's route: the SHA-256 of its relay key.
    public var route: Data
    /// The computer's tunnel identity, pinned: X25519 then ML-KEM-768.
    public var identity: Data
    /// This phone's dial key for the computer's relays (P-256, raw).
    public var dialKey: Data
    /// This phone's tunnel static key for the computer (X25519, raw).
    public var tunnelKey: Data
    /// What the computer's owner calls each relay, keyed by the endpoint's
    /// `description`; refreshed whenever the computer says, without new keys.
    public var labels: [String: RelayLabel]

    public init(
        relays: [RelayEndpoint], route: Data, identity: Data, dialKey: Data, tunnelKey: Data,
        listenerRtt: [String: Int]? = nil, labels: [String: RelayLabel] = [:]
    ) {
        self.relays = relays
        self.listenerRtt = listenerRtt
        self.route = route
        self.identity = identity
        self.dialKey = dialKey
        self.tunnelKey = tunnelKey
        self.labels = labels
    }

    enum CodingKeys: String, CodingKey { case relays, listenerRtt, route, identity, dialKey, tunnelKey, labels }

    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        relays = try c.decode([RelayEndpoint].self, forKey: .relays)
        listenerRtt = try c.decodeIfPresent([String: Int].self, forKey: .listenerRtt)
        route = try c.decode(Data.self, forKey: .route)
        identity = try c.decode(Data.self, forKey: .identity)
        dialKey = try c.decode(Data.self, forKey: .dialKey)
        tunnelKey = try c.decode(Data.self, forKey: .tunnelKey)
        // Records saved before relays had labels have none.
        labels = try c.decodeIfPresent([String: RelayLabel].self, forKey: .labels) ?? [:]
    }

    /// A record for what `info` says, with fresh keys for this phone; nil
    /// when `info` is not a relay answer.
    public static func make(from info: API.RelayInfo) -> RelayRecord? {
        make(from: info, keys: RelayKeyPair())
    }

    /// A record for what `info` says, with keys this phone already sent;
    /// nil when `info` is not a relay answer.
    public static func make(from info: API.RelayInfo, keys: RelayKeyPair) -> RelayRecord? {
        guard let route = Base64URL.decode(info.route), route.count == 32,
              let identity = Base64URL.decode(info.identity),
              (try? Tunnel.Identity(bytes: identity)) != nil
        else { return nil }
        let relays = info.relays.compactMap(RelayEndpoint.init)
        guard !relays.isEmpty else { return nil }
        return RelayRecord(
            relays: relays,
            route: route,
            identity: identity,
            dialKey: keys.dialKey,
            tunnelKey: keys.tunnelKey,
            listenerRtt: info.rttMs,
            labels: labels(from: info)
        )
    }

    /// This record with the order, round trips, and labels `info` reports
    /// now, keys kept, for when `matches(info)`; nil when nothing changed
    /// enough to save: the order and labels are the same and no round trip
    /// moved by more than 20 ms and a quarter, so jitter does not rewrite
    /// the Keychain on every connection.
    public func refreshed(with info: API.RelayInfo) -> RelayRecord? {
        var record = self
        record.relays = info.relays.compactMap(RelayEndpoint.init)
        record.listenerRtt = info.rttMs
        record.labels = Self.labels(from: info)
        guard record.relays == relays, record.labels == labels else { return record }
        let old = listenerRtt ?? [:]
        let new = info.rttMs ?? [:]
        guard Set(old.keys) == Set(new.keys) else { return record }
        let moved = new.contains { name, millis in
            let before = old[name] ?? millis
            let change = abs(millis - before)
            return change > 20 && change * 4 > before
        }
        return moved ? record : nil
    }

    /// What `POST /api/relay` sends: the public halves, base64url.
    public var enrollment: API.RelayKeys? {
        RelayKeyPair(dialKey: dialKey, tunnelKey: tunnelKey).enrollment
    }

    /// Whether `info` still names the same computer and relays, so the
    /// keys already sent hold. Order, round trips, and labels do not count
    /// (`refreshed(with:)`).
    public func matches(_ info: API.RelayInfo) -> Bool {
        Base64URL.decode(info.route) == route
            && Base64URL.decode(info.identity) == identity
            && Set(info.relays.compactMap(RelayEndpoint.init)) == Set(relays)
    }

}

extension API {
    /// `GET /api/relay`.
    public struct RelayInfo: Codable, Hashable, Sendable {
        public let identity: String
        /// Nearest to the computer first.
        public let relays: [String]
        public let route: String
        /// The computer's round trip to each relay it listens on, in
        /// milliseconds; nil from an older daemon.
        public let rttMs: [String: Int]?
        /// What the owner calls each relay, by endpoint; nil from a daemon
        /// without labels, or none to give.
        public let labels: [String: RelayLabel]?

        enum CodingKeys: String, CodingKey {
            case identity, relays, route, labels
            case rttMs = "rtt_ms"
        }

        public init(
            identity: String, relays: [String], route: String,
            rttMs: [String: Int]? = nil, labels: [String: RelayLabel]? = nil
        ) {
            self.identity = identity
            self.relays = relays
            self.route = route
            self.rttMs = rttMs
            self.labels = labels
        }
    }

    /// `POST /api/relay`.
    public struct RelayKeys: Codable, Equatable, Sendable {
        public let dial: String
        public let tunnel: String

        public init(dial: String, tunnel: String) {
            self.dial = dial
            self.tunnel = tunnel
        }
    }
}

/// This phone's private keys for one computer's relays (raw), made before
/// the phone knows the relays when another computer carries them there.
public struct RelayKeyPair: Hashable, Sendable {
    /// The dial key (P-256).
    public let dialKey: Data
    /// The tunnel static key (X25519).
    public let tunnelKey: Data

    /// Fresh keys.
    public init() {
        self.init(
            dialKey: P256.Signing.PrivateKey().rawRepresentation,
            tunnelKey: Curve25519.KeyAgreement.PrivateKey().rawRepresentation
        )
    }

    public init(dialKey: Data, tunnelKey: Data) {
        self.dialKey = dialKey
        self.tunnelKey = tunnelKey
    }

    /// The public halves, base64url, as a computer keeps them.
    public var enrollment: API.RelayKeys? {
        guard let dial = try? P256.Signing.PrivateKey(rawRepresentation: dialKey),
              let tunnel = try? Curve25519.KeyAgreement.PrivateKey(rawRepresentation: tunnelKey)
        else { return nil }
        return API.RelayKeys(
            dial: Base64URL.encode(dial.publicKey.compressedRepresentation),
            tunnel: Base64URL.encode(tunnel.publicKey.rawRepresentation)
        )
    }
}
