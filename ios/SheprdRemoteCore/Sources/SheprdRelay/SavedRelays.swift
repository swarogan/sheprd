import Foundation
import SheprdRemoteCore
import SheprdTunnel

/// The relays this phone keeps for later: the ones its computers named
/// while it reached them directly, and the ones typed in Settings.
///
/// Only typed relays are tried for every computer, after the relays that
/// computer named itself, so a computer that moved to a new relay is still
/// reached once the user adds it here. A relay a computer named is dialed
/// for that computer only: dialing another computer there would tell
/// whoever runs it that computer's route, and when this phone reaches it,
/// though the user never chose that relay for it. Addresses are not
/// secrets: the phone's keys for a computer stay in that computer's
/// Keychain record.
public struct SavedRelays: Codable, Equatable, Sendable {
    /// The most kept: each is one more dial a lost computer costs.
    public static let limit = 16

    public private(set) var relays: [RelayEndpoint]
    /// Those of `relays` the user typed, tried for every computer.
    public private(set) var typed: Set<RelayEndpoint> = []
    /// Read from a version that did not say which were typed, until
    /// `settle` decides. Kept when saved, so a launch that could not read
    /// the paired computers leaves the decision to a later one; until
    /// then no saved relay is tried beyond the computers that named it.
    private var unsettled = false
    /// While unsettled, the relays the user added or stopped sharing
    /// since: `settle` leaves those choices as they are.
    private var decided: Set<RelayEndpoint> = []

    public init(_ relays: [RelayEndpoint] = []) {
        self.relays = []
        learn(relays)
    }

    /// The typed relays, in the order kept: what every computer may be
    /// dialed on beyond its own.
    public var forEveryComputer: [RelayEndpoint] {
        relays.filter(typed.contains)
    }

    /// What became of an address typed in Settings.
    public enum Added: Equatable, Sendable {
        case added(RelayEndpoint)
        /// Already kept.
        case known
        /// Not a relay's address.
        case invalid
        /// `limit` are kept already.
        case full
    }

    /// Keeps the relay at `text`, as typed.
    public mutating func add(_ text: String) -> Added {
        guard !text.trimmingCharacters(in: .whitespaces).isEmpty, let endpoint = RelayEndpoint(text) else {
            return .invalid
        }
        if relays.contains(endpoint) {
            // Typing a relay a computer named makes it the user's choice
            // for every computer.
            decide(endpoint)
            return typed.insert(endpoint).inserted ? .added(endpoint) : .known
        }
        guard relays.count < Self.limit else { return .full }
        relays.append(endpoint)
        typed.insert(endpoint)
        decide(endpoint)
        return .added(endpoint)
    }

    /// Keeps the relays a computer named, up to `limit`. True when any is
    /// new.
    @discardableResult
    public mutating func learn(_ endpoints: [RelayEndpoint]) -> Bool {
        var changed = false
        for endpoint in endpoints where !relays.contains(endpoint) && relays.count < Self.limit {
            relays.append(endpoint)
            changed = true
        }
        return changed
    }

    public mutating func remove(_ endpoint: RelayEndpoint) {
        relays.removeAll { $0 == endpoint }
        typed.remove(endpoint)
    }

    /// Stops trying `endpoint` for every computer, keeping it for the
    /// computers that name it.
    public mutating func unshare(_ endpoint: RelayEndpoint) {
        typed.remove(endpoint)
        decide(endpoint)
    }

    /// Records a choice the user made before an older list was settled.
    private mutating func decide(_ endpoint: RelayEndpoint) {
        if unsettled { decided.insert(endpoint) }
    }

    /// Decides, once, for a list kept before typed relays were told apart:
    /// a relay one of the paired computers names (`named`) is that
    /// computer's; any other was typed, or named by a computer since
    /// removed, and stays for every computer as before, where Settings
    /// shows it and the user can remove it.
    public mutating func settle(named: Set<RelayEndpoint>) {
        guard unsettled else { return }
        unsettled = false
        let undecided = relays.filter { !decided.contains($0) }
        typed.formUnion(undecided.filter { !named.contains($0) })
        decided = []
    }

    private enum CodingKeys: String, CodingKey {
        case decided, relays, typed, unsettled
    }

    public init(from decoder: Decoder) throws {
        // One by one: an address a later version writes and this one
        // cannot read costs that relay, not the list.
        let read = { (texts: [String]) in texts.compactMap(RelayEndpoint.init) }
        if let texts = try? decoder.singleValueContainer().decode([String].self) {
            self.init(read(texts))
            unsettled = true
            return
        }
        let container = try decoder.container(keyedBy: CodingKeys.self)
        self.init(read(try container.decode([String].self, forKey: .relays)))
        let typed = read(try container.decodeIfPresent([String].self, forKey: .typed) ?? [])
        self.typed = Set(typed.filter(relays.contains))
        unsettled = try container.decodeIfPresent(Bool.self, forKey: .unsettled) ?? false
        if unsettled {
            let decided = read(try container.decodeIfPresent([String].self, forKey: .decided) ?? [])
            self.decided = Set(decided.filter(relays.contains))
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(relays.map(\.description), forKey: .relays)
        try container.encode(forEveryComputer.map(\.description), forKey: .typed)
        if unsettled {
            try container.encode(true, forKey: .unsettled)
            try container.encode(relays.filter(decided.contains).map(\.description), forKey: .decided)
        }
    }
}

/// Whether a relay answers, as its `/v1/info` says over HTTPS. Shown in
/// Settings; nothing is decided on it, since a relay that is down now may
/// be back when a computer is dialed.
public enum RelayReach: Equatable, Sendable {
    case reachable
    /// It answers, but not over WebSockets, which this phone dials with.
    case noWebSocket
    /// Something answers there that is not a relay this app speaks to.
    case notARelay
    case unreachable

    struct Info: Decodable {
        let `protocol`: Int
        let websocket: Bool?
    }

    /// What an answer to `GET /v1/info` means.
    public static func read(status: Int, body: Data) -> RelayReach {
        guard status == 200, let info = try? JSONDecoder().decode(Info.self, from: body), info.protocol == 1 else {
            return .notARelay
        }
        // A relay older than the field serves WebSockets.
        return info.websocket == false ? .noWebSocket : .reachable
    }

    /// The most of an answer that is read: a relay's `/v1/info` is a few
    /// hundred bytes.
    static let maxInfo = 16 * 1024

    /// Asks `endpoint` whether it is a relay, waiting at most ten seconds.
    /// The answer is read as it arrives and dropped once it is longer than
    /// `maxInfo`, so whatever a saved address serves, it costs this phone
    /// no more memory than that.
    public static func probe(_ endpoint: RelayEndpoint, session: URLSession = .shared) async -> RelayReach {
        guard let url = endpoint.info else { return .notARelay }
        var request = URLRequest(url: url, cachePolicy: .reloadIgnoringLocalCacheData, timeoutInterval: 10)
        request.httpMethod = "GET"
        guard let (bytes, response) = try? await session.bytes(for: request),
              let http = response as? HTTPURLResponse
        else { return .unreachable }
        // Leaving with the answer unfinished ends the download.
        defer { bytes.task.cancel() }
        guard http.expectedContentLength <= Int64(maxInfo) else { return .notARelay }
        do {
            guard let body = try await bounded(bytes, limit: maxInfo) else { return .notARelay }
            return read(status: http.statusCode, body: body)
        } catch {
            return .unreachable
        }
    }

    /// What `bytes` carries, or nil once it carries more than `limit`: the
    /// rest is never read.
    static func bounded<Bytes: AsyncSequence>(_ bytes: Bytes, limit: Int) async throws -> Data?
        where Bytes.Element == UInt8
    {
        var body = Data()
        for try await byte in bytes {
            guard body.count < limit else { return nil }
            body.append(byte)
        }
        return body
    }
}
