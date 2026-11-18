import Foundation
import SheprdRemoteCore
import SheprdTunnel

/// Which of a computer's relays to try first. A session runs phone to
/// relay to computer, so each relay costs this phone's round trip to it
/// plus the computer's, which `GET /api/relay` reports. Relays share
/// nothing: the phone only ever dials the computer's own relays, so a
/// phone and a computer near different relays still meet, through
/// whichever of the computer's relays makes the shorter path.
public enum RelayNearness {
    /// `relays` (in the computer's order, nearest to it first) ordered by
    /// the whole path. A relay the computer measured comes before one it
    /// did not (it is not listening there now), and one this phone measured
    /// before one it could not reach; a leg not measured adds nothing.
    /// Ties keep the computer's order.
    public static func ranked(
        _ relays: [RelayEndpoint],
        listener: [String: Int],
        phone: [RelayEndpoint: Int]
    ) -> [RelayEndpoint] {
        let anyListener = relays.contains { listener[$0.description] != nil }
        let anyPhone = relays.contains { phone[$0] != nil }
        return relays.enumerated().sorted { left, right in
            key(left.element, left.offset) < key(right.element, right.offset)
        }.map(\.element)

        func key(_ relay: RelayEndpoint, _ index: Int) -> Key {
            let computer = listener[relay.description]
            let mine = phone[relay]
            return Key(
                unlisted: anyListener && computer == nil,
                unprobed: anyPhone && mine == nil,
                total: (computer ?? 0) + (mine ?? 0),
                index: index
            )
        }
    }

    private struct Key: Comparable {
        let unlisted: Bool
        let unprobed: Bool
        let total: Int
        let index: Int

        static func < (left: Key, right: Key) -> Bool {
            (left.unlisted ? 1 : 0, left.unprobed ? 1 : 0, left.total, left.index)
                < (right.unlisted ? 1 : 0, right.unprobed ? 1 : 0, right.total, right.index)
        }
    }
}

/// This phone's round trip to each relay, measured on the side so opening
/// a session never waits for it: the first session after launch uses the
/// computer's order alone, later ones the whole path.
public actor RelayProbes {
    public static let shared = RelayProbes()

    /// How long a measurement is used: the phone moves between networks.
    static let fresh: TimeInterval = 5 * 60
    static let timeout: TimeInterval = 3

    private var measured: [RelayEndpoint: (millis: Int, at: Date)] = [:]
    /// The probes running, each with the network it started on.
    private var probing: [RelayEndpoint: Int] = [:]
    /// Which network measurements are for, bumped by `forget`: a probe
    /// that started on an earlier one is not recorded.
    private var network = 0

    /// The fresh measurements among `relays`, in milliseconds.
    public func rtt(_ relays: [RelayEndpoint], now: Date = Date()) -> [RelayEndpoint: Int] {
        var fresh: [RelayEndpoint: Int] = [:]
        for relay in relays {
            if let entry = measured[relay], now.timeIntervalSince(entry.at) < Self.fresh {
                fresh[relay] = entry.millis
            }
        }
        return fresh
    }

    /// Records a measurement, as a probe does; tests set them directly.
    public func record(_ relay: RelayEndpoint, millis: Int, at: Date = Date()) {
        measured[relay] = (max(1, millis), at)
    }

    /// Forgets every measurement, when the phone changed networks. Probes
    /// still running are for the old network: their results are dropped,
    /// and the next `refresh` starts new ones.
    public func forget() {
        measured.removeAll()
        probing.removeAll()
        network += 1
    }

    /// Measures, in the background, each of `relays` with no fresh
    /// measurement and no probe running.
    public func refresh(_ relays: [RelayEndpoint], session: URLSession = .shared) {
        let fresh = rtt(relays)
        for relay in relays where fresh[relay] == nil && probing[relay] == nil {
            let network = network
            probing[relay] = network
            Task {
                let millis = await Self.probe(relay, session: session)
                self.finished(relay, millis: millis, network: network)
            }
        }
    }

    /// Records a probe's result, unless the phone changed networks since
    /// it started.
    func finished(_ relay: RelayEndpoint, millis: Int?, network: Int) {
        guard network == self.network else { return }
        probing[relay] = nil
        if let millis { record(relay, millis: millis) }
    }

    /// The network measurements are for now; tests start probes on it.
    var currentNetwork: Int { network }

    /// The round trip of a second `GET /v1/info`, once the first has set up
    /// the connection, so only the path is timed, not TLS.
    static func probe(_ relay: RelayEndpoint, session: URLSession) async -> Int? {
        guard let url = relay.info else { return nil }
        var request = URLRequest(url: url, cachePolicy: .reloadIgnoringLocalCacheData, timeoutInterval: timeout)
        request.httpMethod = "GET"
        guard await answered(request, session: session) else { return nil }
        let start = Date()
        guard await answered(request, session: session) else { return nil }
        return Int(Date().timeIntervalSince(start) * 1_000)
    }

    /// Whether `request` got a 200 of at most `RelayReach.maxInfo` bytes.
    /// The answer is read as it arrives and dropped past that, as
    /// `RelayReach.probe` does, so whatever a relay serves costs this phone
    /// no more memory than that.
    static func answered(_ request: URLRequest, session: URLSession) async -> Bool {
        guard let (bytes, response) = try? await session.bytes(for: request),
              let http = response as? HTTPURLResponse
        else { return false }
        // Leaving with the answer unfinished ends the download.
        defer { bytes.task.cancel() }
        guard http.statusCode == 200, http.expectedContentLength <= Int64(RelayReach.maxInfo) else { return false }
        return (try? await RelayReach.bounded(bytes, limit: RelayReach.maxInfo)) != nil
    }
}
