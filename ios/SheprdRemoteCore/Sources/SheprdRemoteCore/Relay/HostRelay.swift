import CryptoKit
import Foundation
import Security

/// Carrying this phone's relay keys to a computer it cannot reach, through
/// one it can (`crates/sheprd-remote-control/src/control/host_relay.rs`).
/// The phone signs the keys with the device key it holds for the far
/// computer and posts them to the near one (`POST /api/v2/hosts/relay`),
/// whose Sheprd finds the far computer among its SSH and cloud hosts.
public enum HostRelay {
    /// What the phone signs, before the fields of `message`.
    public static let prefix = Data("sheprd-host-relay-v1".utf8)
    /// What the far computer signs, before the request's message and its
    /// answer.
    public static let answerPrefix = Data("sheprd-host-relay-answer-v1".utf8)

    /// What the phone signs: the prefix, the far computer's fingerprint,
    /// then the device id, the dial key, and the tunnel key as their wire
    /// text, each after its length as a big-endian `UInt16`, then
    /// `signedMs` as a big-endian `UInt64`. Nil when a field is too long.
    public static func message(fingerprint: Fingerprint, device: String, keys: API.RelayKeys, signedMs: UInt64) -> Data? {
        var message = prefix + fingerprint.bytes
        for text in [device, keys.dial, keys.tunnel] {
            let bytes = Data(text.utf8)
            guard let length = UInt16(exactly: bytes.count) else { return nil }
            message.append(contentsOf: [UInt8(length >> 8), UInt8(length & 0xff)])
            message.append(bytes)
        }
        message.append(contentsOf: (0..<8).reversed().map { UInt8((signedMs >> ($0 * 8)) & 0xff) })
        return message
    }

    /// What the far computer signs for `request`: the answer prefix, the
    /// request's message, then `1` and the relays' identity, route, count
    /// (big-endian `UInt16`), and each relay, every text after its length
    /// as a big-endian `UInt16`; or `2` for keys kept without relays (nil
    /// `info`).
    public static func answerMessage(for request: API.HostRelayRequest, info: API.RelayInfo?) -> Data? {
        guard let fingerprint = Fingerprint(request.fingerprint),
              let signed = message(fingerprint: fingerprint, device: request.device, keys: request.keys, signedMs: request.signedMs)
        else { return nil }
        var message = answerPrefix + signed
        guard let info else {
            message.append(2)
            return message
        }
        message.append(1)
        func push(_ text: String) -> Bool {
            let bytes = Data(text.utf8)
            guard let length = UInt16(exactly: bytes.count) else { return false }
            message.append(contentsOf: [UInt8(length >> 8), UInt8(length & 0xff)])
            message.append(bytes)
            return true
        }
        guard push(info.identity), push(info.route), let count = UInt16(exactly: info.relays.count) else { return nil }
        message.append(contentsOf: [UInt8(count >> 8), UInt8(count & 0xff)])
        for relay in info.relays where !push(relay) { return nil }
        return message
    }

    /// Whether an answer comes from the computer `request` names: its
    /// certificate hashes to the request's fingerprint, and that
    /// certificate's key signed the answer. Any other host Sheprd asked
    /// could claim to be it.
    public static func proven(_ proof: API.RelayProof, info: API.RelayInfo?, for request: API.HostRelayRequest) -> Bool {
        guard let der = Base64URL.decode(proof.certificate),
              let pinned = Fingerprint(request.fingerprint),
              Fingerprint(certificate: der) == pinned,
              let message = answerMessage(for: request, info: info),
              let raw = Base64URL.decode(proof.signature),
              let signature = try? P256.Signing.ECDSASignature(rawRepresentation: raw),
              let certificate = SecCertificateCreateWithData(nil, der as CFData),
              let key = SecCertificateCopyKey(certificate),
              let point = SecKeyCopyExternalRepresentation(key, nil) as Data?,
              let publicKey = try? P256.Signing.PublicKey(x963Representation: point)
        else { return false }
        return publicKey.isValidSignature(signature, for: message)
    }
}

extension API {
    /// `POST /api/v2/hosts/relay`: this phone's relay keys for the far
    /// computer, signed with its key there.
    public struct HostRelayRequest: Codable, Equatable, Sendable {
        /// The far computer's fingerprint.
        public let fingerprint: String
        /// This phone's device id on the far computer.
        public let device: String
        public let keys: RelayKeys
        public let signedMs: UInt64
        /// The signature over `HostRelay.message`, `r || s`, base64url.
        public let signature: String

        public init(fingerprint: String, device: String, keys: RelayKeys, signedMs: UInt64, signature: String) {
            self.fingerprint = fingerprint
            self.device = device
            self.keys = keys
            self.signedMs = signedMs
            self.signature = signature
        }

        enum CodingKeys: String, CodingKey {
            case fingerprint, device, keys, signature
            case signedMs = "signed_ms"
        }
    }

    /// The far computer's signature over its answer: its CA certificate
    /// (DER), that certificate's key, and the signature, base64url.
    public struct RelayProof: Codable, Equatable, Sendable {
        public let certificate: String
        public let key: String
        public let signature: String

        public init(certificate: String, key: String, signature: String) {
            self.certificate = certificate
            self.key = key
            self.signature = signature
        }
    }

    /// Where a carried request stands, tagged by `state` and, once done,
    /// `outcome`.
    public enum HostRelayState: Decodable, Equatable, Sendable {
        /// Waiting for Sheprd on the near computer.
        case pending
        /// The far computer keeps the keys and listens on these relays.
        case relayed(RelayInfo, RelayProof)
        /// The far computer keeps the keys but listens on no relay yet.
        case kept(RelayProof)
        /// No host the near computer's Sheprd connects to is the far one.
        case elsewhere
        case failed(reason: String)

        enum CodingKeys: String, CodingKey { case state, outcome, reason, proof }

        public init(from decoder: Decoder) throws {
            let container = try decoder.container(keyedBy: CodingKeys.self)
            if try container.decode(String.self, forKey: .state) == "pending" {
                self = .pending
                return
            }
            let outcome = try container.decode(String.self, forKey: .outcome)
            switch outcome {
            case "relayed":
                self = .relayed(try RelayInfo(from: decoder), try container.decode(RelayProof.self, forKey: .proof))
            case "kept": self = .kept(try container.decode(RelayProof.self, forKey: .proof))
            case "elsewhere": self = .elsewhere
            case "failed": self = .failed(reason: try container.decode(String.self, forKey: .reason))
            default:
                throw DecodingError.dataCorruptedError(
                    forKey: .outcome, in: container, debugDescription: "unknown relay outcome \(outcome)"
                )
            }
        }
    }
}

/// Which computers this phone asks another to carry its relay keys to, and
/// through which. A computer is due when its link is not online, it has no
/// relay record, and no attempt for it is running or finished recently. The
/// near computer is the one that vouched for it when that one is online,
/// else any online computer not tried for it recently.
public struct HostRelayPlan: Sendable {
    /// How long a computer waits after an attempt through a near computer
    /// before that near one is asked again.
    public static let retry: TimeInterval = 30 * 60
    /// How long a computer waits after an answer that settles it for now:
    /// it keeps the keys but listens on no relay, or no computer online
    /// connects to it. Each attempt has Sheprd try its machines over SSH.
    public static let settled: TimeInterval = 24 * 60 * 60

    /// Far computer, then near computer, to when it was last asked.
    private var asked: [String: [String: Date]] = [:]
    /// Far computers to leave alone until then.
    private var quiet: [String: Date] = [:]
    private var running: Set<String> = []

    public init() {}

    /// One computer that can carry keys to one that cannot be reached.
    public struct Pick: Equatable, Sendable {
        public let far: Computer
        public let near: Computer
    }

    /// The attempts to start now, at most one per far computer; each is
    /// marked running until `finished`. Only a computer whose link gave up
    /// is far: one still connecting may yet answer directly.
    public mutating func due(_ machines: [(computer: Computer, status: LinkStatus)], at now: Date) -> [Pick] {
        let online = machines.filter { if case .online = $0.status { true } else { false } }.map(\.computer)
        var picks: [Pick] = []
        for (far, status) in machines where far.relay == nil && !running.contains(far.device) {
            guard case .offline = status, quiet[far.device].map({ now >= $0 }) ?? true else { continue }
            let tried = asked[far.device] ?? [:]
            let candidates = online.filter { near in
                near.device != far.device
                    && tried[near.device].map { now.timeIntervalSince($0) >= Self.retry } ?? true
            }
            // The computer that paired this one knows it best.
            guard let near = candidates.first(where: { $0.fingerprint == far.via }) ?? candidates.first else { continue }
            asked[far.device, default: [:]][near.device] = now
            running.insert(far.device)
            picks.append(Pick(far: far, near: near))
        }
        return picks
    }

    /// The attempt for `far` ended; `settled` when its answer holds for a
    /// while (`Self.settled`).
    public mutating func finished(_ far: String, settled: Bool = false, at now: Date) {
        running.remove(far)
        if settled { quiet[far] = now + Self.settled }
    }

    /// `far` was forgotten or reached: its attempts start over.
    public mutating func forget(_ far: String) {
        asked[far] = nil
        quiet[far] = nil
        running.remove(far)
    }
}
