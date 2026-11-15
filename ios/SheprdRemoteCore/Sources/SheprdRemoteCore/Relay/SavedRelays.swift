import Foundation

/// The relays this phone keeps for later: the ones its computers named
/// while it reached them directly, and the ones typed in Settings. They
/// are tried for every computer after the relays that computer named
/// itself, so a computer that moved to a new relay is still reached once
/// its address is known here. Addresses are not secrets: the phone's keys
/// for a computer stay in that computer's Keychain record.
public struct SavedRelays: Codable, Equatable, Sendable {
    /// The most kept: each is one more dial a lost computer costs.
    public static let limit = 16

    public private(set) var relays: [RelayEndpoint]

    public init(_ relays: [RelayEndpoint] = []) {
        self.relays = []
        learn(relays)
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
        if relays.contains(endpoint) { return .known }
        guard relays.count < Self.limit else { return .full }
        relays.append(endpoint)
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
    }

    public init(from decoder: Decoder) throws {
        // One by one: an address a later version writes and this one
        // cannot read costs that relay, not the list.
        let texts = try decoder.singleValueContainer().decode([String].self)
        self.init(texts.compactMap(RelayEndpoint.init))
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        try container.encode(relays.map(\.description))
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
