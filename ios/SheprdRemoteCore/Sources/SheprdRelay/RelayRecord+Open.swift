import CryptoKit
import Foundation
import SheprdRemoteCore
import SheprdTunnel

/// Reaching the computer a saved `RelayRecord` names: ranking its relays by
/// round trip, and opening a session through the first that answers.
extension RelayRecord {
    /// The relays to try, best path first, given this phone's own round
    /// trips (`RelayProbes`).
    public func ranked(phone: [RelayEndpoint: Int]) -> [RelayEndpoint] {
        RelayNearness.ranked(relays, listener: listenerRtt ?? [:], phone: phone)
    }

    /// The relays to try now, and measures this phone's legs for next time.
    private func order(session: URLSession) async -> [RelayEndpoint] {
        let probes = RelayProbes.shared
        let phone = await probes.rtt(relays)
        await probes.refresh(relays, session: session)
        return ranked(phone: phone)
    }

    /// Opens the pairing session a nearby offer allowed, through the first
    /// relay that answers, best path first: the one-dial key and the
    /// tunnel's pairing PSK both come from the offer's token, `secret`.
    public func openPairing(secret: Data, session: URLSession = .shared) async throws -> RelaySession {
        let dial = RelayProof.pairingDialKey(secret: secret)
        let tunnel = try Curve25519.KeyAgreement.PrivateKey(rawRepresentation: tunnelKey)
        let listener = try Tunnel.Identity(bytes: identity)
        var last: Error = RelaySession.Failure.closed
        for relay in await order(session: session) {
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

    /// The relays to dial the computer on, in order: the ones it named,
    /// then those of `saved` it did not. Its route and pinned identity
    /// are the same on every relay, so a saved relay it does not listen on
    /// only answers that it is offline.
    public func candidates(saved: [RelayEndpoint]) -> [RelayEndpoint] {
        relays + saved.filter { !relays.contains($0) }
    }

    /// Opens a session to the computer through the first relay that
    /// answers: its own best path first, then those of `saved`. A relay
    /// that says nothing holds the next one back for `RelayRace.stagger`
    /// only, so a computer that moved off a dead relay is still reached
    /// through a saved one.
    public func open(saved: [RelayEndpoint] = [], session: URLSession = .shared) async throws -> RelaySession {
        let dial = try P256.Signing.PrivateKey(rawRepresentation: dialKey)
        let tunnel = try Curve25519.KeyAgreement.PrivateKey(rawRepresentation: tunnelKey)
        let listener = try Tunnel.Identity(bytes: identity)
        let ordered = await order(session: session)
        let route = route
        return try await RelayRace.first(
            of: ordered + saved.filter { !ordered.contains($0) },
            none: RelaySession.Failure.closed,
            open: { relay in
                try await RelaySession.open(
                    endpoint: relay, route: route, dialKey: dial,
                    tunnelKey: tunnel, listener: listener, session: session
                )
            },
            discard: { await $0.close() }
        )
    }
}
