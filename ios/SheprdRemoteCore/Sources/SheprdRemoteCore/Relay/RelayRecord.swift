import CryptoKit
import Foundation

/// How this phone reaches one computer through its relays: what the
/// computer said over a path the phone already trusted (`GET /api/relay`),
/// and the keys the phone made for it and sent (`POST /api/relay`). Kept
/// in the computer's Keychain record, as its device key is.
public struct RelayRecord: Codable, Hashable, Sendable {
    /// The relays the computer listens on.
    public var relays: [RelayEndpoint]
    /// The computer's route: the SHA-256 of its relay key.
    public var route: Data
    /// The computer's tunnel identity, pinned: X25519 then ML-KEM-768.
    public var identity: Data
    /// This phone's dial key for the computer's relays (P-256, raw).
    public var dialKey: Data
    /// This phone's tunnel static key for the computer (X25519, raw).
    public var tunnelKey: Data

    public init(relays: [RelayEndpoint], route: Data, identity: Data, dialKey: Data, tunnelKey: Data) {
        self.relays = relays
        self.route = route
        self.identity = identity
        self.dialKey = dialKey
        self.tunnelKey = tunnelKey
    }

    /// A record for what `info` says, with fresh keys for this phone; nil
    /// when `info` is not a relay answer.
    public static func make(from info: API.RelayInfo) -> RelayRecord? {
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
            dialKey: P256.Signing.PrivateKey().rawRepresentation,
            tunnelKey: Curve25519.KeyAgreement.PrivateKey().rawRepresentation
        )
    }

    /// What `POST /api/relay` sends: the public halves, base64url.
    public var enrollment: API.RelayKeys? {
        guard let dial = try? P256.Signing.PrivateKey(rawRepresentation: dialKey),
              let tunnel = try? Curve25519.KeyAgreement.PrivateKey(rawRepresentation: tunnelKey)
        else { return nil }
        return API.RelayKeys(
            dial: Base64URL.encode(dial.publicKey.compressedRepresentation),
            tunnel: Base64URL.encode(tunnel.publicKey.rawRepresentation)
        )
    }

    /// Whether `info` still names the same computer and relays, so the
    /// keys already sent hold.
    public func matches(_ info: API.RelayInfo) -> Bool {
        Base64URL.decode(info.route) == route
            && Base64URL.decode(info.identity) == identity
            && Set(info.relays.compactMap(RelayEndpoint.init)) == Set(relays)
    }

    /// Opens the pairing session a nearby offer allowed, through the first
    /// relay that answers: the one-dial key and the tunnel's pairing PSK
    /// both come from the offer's token, `secret`.
    public func openPairing(secret: Data, session: URLSession = .shared) async throws -> RelaySession {
        let dial = RelayProof.pairingDialKey(secret: secret)
        let tunnel = try Curve25519.KeyAgreement.PrivateKey(rawRepresentation: tunnelKey)
        let listener = try Tunnel.Identity(bytes: identity)
        var last: Error = RelaySession.Failure.closed
        for relay in relays {
            do {
                return try await RelaySession.open(
                    endpoint: relay, route: route, dialKey: dial, tunnel: .pairing(local: tunnel, listener: listener, secret: secret),
                    session: session
                )
            } catch {
                last = error
            }
        }
        throw last
    }

    /// Opens a session to the computer through the first relay that
    /// answers.
    public func open(session: URLSession = .shared) async throws -> RelaySession {
        let dial = try P256.Signing.PrivateKey(rawRepresentation: dialKey)
        let tunnel = try Curve25519.KeyAgreement.PrivateKey(rawRepresentation: tunnelKey)
        let listener = try Tunnel.Identity(bytes: identity)
        var last: Error = RelaySession.Failure.closed
        for relay in relays {
            do {
                return try await RelaySession.open(
                    endpoint: relay, route: route, dialKey: dial,
                    tunnelKey: tunnel, listener: listener, session: session
                )
            } catch {
                last = error
            }
        }
        throw last
    }
}

extension API {
    /// `GET /api/relay`.
    public struct RelayInfo: Codable, Hashable, Sendable {
        public let identity: String
        public let relays: [String]
        public let route: String

        public init(identity: String, relays: [String], route: String) {
            self.identity = identity
            self.relays = relays
            self.route = route
        }
    }

    /// `POST /api/relay`.
    public struct RelayKeys: Codable, Equatable, Sendable {
        public let dial: String
        public let tunnel: String
    }
}
