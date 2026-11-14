import CryptoKit
import Foundation
import Testing
@testable import SheprdRemoteCore

/// The daemon's Rust tests write these vectors (`sheprd-tunnel`,
/// `tests/vectors.rs`): every key and random value fixed, so the Swift
/// dialer must produce the same bytes and open the same records.
struct TunnelVectorTests {
    struct Record: Decodable {
        let from: String
        let data: String?
        let close: Bool?
        let record: String
    }

    struct Session: Decodable {
        let protocol_: String
        let dialer_static: String
        let dialer_public: String
        let dialer_ephemeral: String
        let listener_mlkem_seed: String
        let listener_identity: String
        let listener_fingerprint: String
        let route: String
        let prologue: String
        let mlkem_ciphertext: String
        let mlkem_shared: String
        let first: String
        let answer: String
        let handshake_hash: String
        let records: [Record]

        enum CodingKeys: String, CodingKey {
            case protocol_ = "protocol"
            case dialer_static, dialer_public, dialer_ephemeral, listener_mlkem_seed, listener_identity
            case listener_fingerprint, route, prologue, mlkem_ciphertext, mlkem_shared, first, answer
            case handshake_hash, records
        }
    }

    struct Pairing: Decodable {
        let dialer_static: String
        let dialer_ephemeral: String
        let listener_public: String
        let listener_fingerprint: String
        let route: String
        let secret: String
        let psk: String
        let prologue: String
        let first: String
        let answer: String
        let handshake_hash: String
        let code: UInt32
        let records: [Record]
    }

    static func load<Value: Decodable>(_ name: String, as type: Value.Type) throws -> Value {
        let url = try #require(Bundle.module.url(forResource: "Fixtures/tunnel/\(name)", withExtension: "json"))
        return try JSONDecoder().decode(type, from: Data(contentsOf: url))
    }

    static func hex(_ text: String) throws -> Data {
        var bytes = Data()
        var index = text.startIndex
        while index < text.endIndex {
            let next = text.index(index, offsetBy: 2)
            bytes.append(try #require(UInt8(text[index..<next], radix: 16)))
            index = next
        }
        return bytes
    }

    static func key(_ text: String) throws -> Curve25519.KeyAgreement.PrivateKey {
        try Curve25519.KeyAgreement.PrivateKey(rawRepresentation: hex(text))
    }

    /// Seals the dialer's records and opens the listener's, as the vectors
    /// list them, ten minutes passing where the rekey falls.
    static func replay(_ records: [Record], on transport: inout Tunnel.Transport) throws {
        let start = Date(timeIntervalSince1970: 1_800_000_000)
        var sealed = 0
        for record in records {
            let expected = try hex(record.record)
            if record.close == true {
                #expect(try transport.sealClose() == expected)
                continue
            }
            let data = try hex(try #require(record.data))
            if record.from == "dialer" {
                // The fourth and fifth records were sealed ten minutes on.
                let at = sealed >= 2 ? start.addingTimeInterval(Tunnel.Transport.rekeyAfter) : start
                #expect(try transport.seal(data, now: at) == expected)
                sealed += 1
            } else {
                #expect(try transport.open(expected) == .data(data))
            }
        }
    }

    @Test func aSessionMatchesTheDaemonByteForByte() throws {
        let vector = try Self.load("session", as: Session.self)
        #expect(vector.protocol_ == Noise.protocolName)
        let identity = try Tunnel.Identity(bytes: Self.hex(vector.listener_identity))
        #expect(identity.fingerprint == (try Self.hex(vector.listener_fingerprint)))
        let local = try Self.key(vector.dialer_static)
        #expect(local.publicKey.rawRepresentation == (try Self.hex(vector.dialer_public)))
        #expect(Tunnel.prologue(.session, route: try Self.hex(vector.route), listener: identity.fingerprint) == (try Self.hex(vector.prologue)))
        let ciphertext = try Self.hex(vector.mlkem_ciphertext)
        let shared = try Self.hex(vector.mlkem_shared)
        let (initiator, first) = try Tunnel.Initiator.session(
            local: local,
            listener: identity,
            route: Self.hex(vector.route),
            ephemeral: Self.key(vector.dialer_ephemeral),
            encapsulate: { _ in (ciphertext, shared) }
        )
        #expect(first == (try Self.hex(vector.first)))
        var (transport, hash) = try initiator.finish(answer: Self.hex(vector.answer))
        #expect(hash == (try Self.hex(vector.handshake_hash)))
        try Self.replay(vector.records, on: &transport)
    }

    @Test func cryptoKitsMLKEMMatchesTheDaemons() throws {
        // The listener's key from its seed, as `ml-kem` makes it, and the
        // vector's ciphertext opening to the same secret.
        let vector = try Self.load("session", as: Session.self)
        let identity = try Tunnel.Identity(bytes: Self.hex(vector.listener_identity))
        let privateKey = try MLKEM768.PrivateKey(seedRepresentation: Self.hex(vector.listener_mlkem_seed), publicKey: nil)
        #expect(privateKey.publicKey.rawRepresentation == identity.mlkem)
        let shared = try privateKey.decapsulate(Self.hex(vector.mlkem_ciphertext))
        #expect(shared.data == (try Self.hex(vector.mlkem_shared)))
    }

    @Test func pairingMatchesTheDaemonAndShowsItsCode() throws {
        let vector = try Self.load("pairing", as: Pairing.self)
        #expect(Tunnel.pairingPSK(secret: try Self.hex(vector.secret)) == (try Self.hex(vector.psk)))
        let (initiator, first) = try Tunnel.Initiator.pairing(
            local: Self.key(vector.dialer_static),
            listenerX25519: Self.hex(vector.listener_public),
            listenerFingerprint: Self.hex(vector.listener_fingerprint),
            route: Self.hex(vector.route),
            secret: Self.hex(vector.secret),
            ephemeral: Self.key(vector.dialer_ephemeral)
        )
        #expect(first == (try Self.hex(vector.first)))
        var (transport, hash) = try initiator.finish(answer: Self.hex(vector.answer))
        #expect(hash == (try Self.hex(vector.handshake_hash)))
        #expect(Tunnel.pairingCode(handshakeHash: hash) == vector.code)
        try Self.replay(vector.records, on: &transport)
    }

    @Test func aChangedAnswerOrWrongSecretIsRefused() throws {
        let vector = try Self.load("pairing", as: Pairing.self)
        var answer = try Self.hex(vector.answer)
        answer[answer.startIndex + 40] ^= 1
        let (initiator, _) = try Tunnel.Initiator.pairing(
            local: Self.key(vector.dialer_static),
            listenerX25519: Self.hex(vector.listener_public),
            listenerFingerprint: Self.hex(vector.listener_fingerprint),
            route: Self.hex(vector.route),
            secret: Self.hex(vector.secret),
            ephemeral: Self.key(vector.dialer_ephemeral)
        )
        #expect(throws: Tunnel.Failure.handshake) { try initiator.finish(answer: answer) }
        let (stranger, _) = try Tunnel.Initiator.pairing(
            local: Self.key(vector.dialer_static),
            listenerX25519: Self.hex(vector.listener_public),
            listenerFingerprint: Self.hex(vector.listener_fingerprint),
            route: Self.hex(vector.route),
            secret: Data(count: 32),
            ephemeral: Self.key(vector.dialer_ephemeral)
        )
        #expect(throws: Tunnel.Failure.handshake) { try stranger.finish(answer: Self.hex(vector.answer)) }
    }
}

struct TunnelFrameTests {
    @Test func framesComeOutWholeHoweverBytesArrive() throws {
        let messages = [Data("hello".utf8), Data(), Data(repeating: 9, count: 7_000)]
        let stream = try messages.map { try Tunnel.Frame.encode($0) }.reduce(Data(), +)
        for chunk in [1, 3, 1_000, stream.count] {
            var decoder = Tunnel.Frame.Decoder()
            var out: [Data] = []
            var offset = 0
            while offset < stream.count {
                let end = min(offset + chunk, stream.count)
                decoder.push(stream[offset..<end])
                offset = end
                while let frame = decoder.next() { out.append(frame) }
            }
            #expect(out == messages)
        }
        #expect(throws: Tunnel.Failure.frameTooLong) { try Tunnel.Frame.encode(Data(count: 65_536)) }
    }
}
