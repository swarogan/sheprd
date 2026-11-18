import Foundation

/// A chat's summary as its row shows it: what the chat's own agent, run
/// once on its own, said of where the chat stands.
public enum RowSummary: Codable, Hashable, Sendable {
    case pending(sinceMs: UInt64)
    case ready(text: String, atMs: UInt64)
    /// Why there is none, as the computer words it.
    case failed(reason: String, atMs: UInt64)
    /// A state from a newer computer.
    case other

    private enum CodingKeys: String, CodingKey {
        case state, text, reason
        case sinceMs = "since_ms"
        case atMs = "at_ms"
    }

    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        switch try c.decode(String.self, forKey: .state) {
        case "pending": self = .pending(sinceMs: try c.decode(UInt64.self, forKey: .sinceMs))
        case "ready": self = .ready(text: try c.decode(String.self, forKey: .text), atMs: try c.decode(UInt64.self, forKey: .atMs))
        case "failed": self = .failed(reason: try c.decode(String.self, forKey: .reason), atMs: try c.decode(UInt64.self, forKey: .atMs))
        default: self = .other
        }
    }

    public func encode(to encoder: Encoder) throws {
        var c = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case let .pending(since):
            try c.encode("pending", forKey: .state)
            try c.encode(since, forKey: .sinceMs)
        case let .ready(text, at):
            try c.encode("ready", forKey: .state)
            try c.encode(text, forKey: .text)
            try c.encode(at, forKey: .atMs)
        case let .failed(reason, at):
            try c.encode("failed", forKey: .state)
            try c.encode(reason, forKey: .reason)
            try c.encode(at, forKey: .atMs)
        case .other: try c.encode("other", forKey: .state)
        }
    }
}

extension Home.Summary {
    /// The summary while it still says where the chat stands: one the agent
    /// has moved past, by ending a turn after it, gives the row back to the
    /// latest reply.
    public var currentRowSummary: RowSummary? {
        switch rowSummary {
        case .pending?: return rowSummary
        case let .ready(_, at)?, let .failed(_, at)?:
            if let ended = lastTurnEndedMs, ended > at { return nil }
            return rowSummary
        case .other?, nil: return nil
        }
    }
}
