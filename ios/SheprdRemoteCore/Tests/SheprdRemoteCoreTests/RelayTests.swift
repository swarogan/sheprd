import CryptoKit
import Foundation
import Testing
@testable import SheprdRemoteCore

/// `sheprd-relay`'s tests write these vectors (`tests/ios_vectors.rs`).
struct RelayVectorTests {
    struct Endpoint: Decodable {
        let text: String
        let proof_name: String
        let dial: String
    }

    struct Dial: Decodable {
        let endpoints: [Endpoint]
        let proof_name: String
        let nonce: String
        let route: String
        let route_text: String
        let signed: String
    }

    static func load() throws -> Dial {
        let url = try #require(Bundle.module.url(forResource: "Fixtures/relay/dial", withExtension: "json"))
        return try JSONDecoder().decode(Dial.self, from: Data(contentsOf: url))
    }

    @Test func endpointsReadAsTheRelayCrateReadsThem() throws {
        let vector = try Self.load()
        for expected in vector.endpoints {
            let endpoint = try #require(RelayEndpoint(expected.text), "\(expected.text)")
            #expect(endpoint.proofName == expected.proof_name, "\(expected.text)")
            #expect(endpoint.dial(route: Data(repeating: 7, count: 32))?.absoluteString == expected.dial, "\(expected.text)")
        }
        #expect(RelayEndpoint("http://relay.example.org") == nil)
        #expect(RelayEndpoint("ftp://relay.example.org") == nil)
        #expect(RelayEndpoint("https://user@relay.example.org") == nil)
    }

    @Test func aDialerSignsTheBytesTheRelayChecks() throws {
        let vector = try Self.load()
        let nonce = try TunnelVectorTests.hex(vector.nonce)
        let route = try TunnelVectorTests.hex(vector.route)
        let signed = RelayProof.dialSigned(name: vector.proof_name, nonce: nonce, route: route)
        #expect(signed == (try TunnelVectorTests.hex(vector.signed)))
        #expect(Base64URL.encode(route) == vector.route_text)
        let key = P256.Signing.PrivateKey()
        let hello = try RelayProof.hello(key: key, signed: signed)
        let keyText = try #require(hello["key"])
        let signatureText = try #require(hello["signature"])
        let keyBytes = try #require(Base64URL.decode(keyText))
        let signatureBytes = try #require(Base64URL.decode(signatureText))
        #expect(keyBytes.count == 33)
        let publicKey = try P256.Signing.PublicKey(compressedRepresentation: keyBytes)
        let signature = try P256.Signing.ECDSASignature(rawRepresentation: signatureBytes)
        #expect(publicKey.isValidSignature(signature, for: signed))
    }

    @Test func anEndpointRoundTripsThroughJSON() throws {
        let endpoint = try #require(RelayEndpoint("relay.example.org:8443/sheprd/"))
        let data = try JSONEncoder().encode(endpoint)
        #expect(try JSONDecoder().decode(RelayEndpoint.self, from: data) == endpoint)
        #expect(endpoint.description == "https://relay.example.org:8443/sheprd")
    }
}

struct RelayRecordTests {
    static func info() throws -> API.RelayInfo {
        let vector = try TunnelVectorTests.load("session", as: TunnelVectorTests.Session.self)
        let identity = try TunnelVectorTests.hex(vector.listener_identity)
        return API.RelayInfo(
            identity: Base64URL.encode(identity),
            relays: ["https://161-35-226-62.sslip.io", "nonsense://x"],
            route: Base64URL.encode(Data(repeating: 0x52, count: 32))
        )
    }

    @Test func aRecordIsMadeFromWhatTheComputerSaidWithFreshKeys() throws {
        let info = try Self.info()
        let record = try #require(RelayRecord.make(from: info))
        #expect(record.relays.map(\.description) == ["https://161-35-226-62.sslip.io"])
        #expect(record.matches(info))
        let other = try #require(RelayRecord.make(from: info))
        #expect(record.dialKey != other.dialKey)
        #expect(record.tunnelKey != other.tunnelKey)
        let enrollment = try #require(record.enrollment)
        let dial = try P256.Signing.PrivateKey(rawRepresentation: record.dialKey)
        #expect(Base64URL.decode(enrollment.dial) == dial.publicKey.compressedRepresentation)
        #expect(Base64URL.decode(enrollment.tunnel)?.count == 32)
        // A computer that moved to other relays, or changed keys, does not match.
        let moved = API.RelayInfo(identity: info.identity, relays: ["https://elsewhere.example.org"], route: info.route)
        #expect(!record.matches(moved))
        #expect(RelayRecord.make(from: API.RelayInfo(identity: "x", relays: info.relays, route: info.route)) == nil)
        #expect(RelayRecord.make(from: API.RelayInfo(identity: info.identity, relays: [], route: info.route)) == nil)
    }

    @Test func computersKeepTheirRelayAndOldOnesHaveNone() throws {
        let record = try #require(RelayRecord.make(from: Self.info()))
        let computer = Computer(
            device: "d_1", name: "Studio", origins: ["https://192.168.1.20:7443"],
            fingerprint: try #require(Fingerprint(String(repeating: "AB", count: 32))),
            keyKind: .software, keyData: Data(repeating: 1, count: 32), relay: record
        )
        let saved = try JSONEncoder().encode(computer)
        #expect(try JSONDecoder().decode(Computer.self, from: saved).relay == record)
        var old = try #require(try JSONSerialization.jsonObject(with: saved) as? [String: Any])
        old["relay"] = nil
        let older = try JSONSerialization.data(withJSONObject: old)
        #expect(try JSONDecoder().decode(Computer.self, from: older).relay == nil)
        old["relay"] = ["route": "not a record"]
        let broken = try JSONSerialization.data(withJSONObject: old)
        #expect(try JSONDecoder().decode(Computer.self, from: broken).relay == nil)
    }
}

/// Against the real relay and a computer behind it, when one runs:
/// `cargo run -p sheprd-relay --example echo_computer --features
/// client,server` prints the line to put in `SHEPRD_RELAY_ECHO`.
struct RelayLiveTests {
    struct Echo: Decodable {
        let endpoint: String
        let route: String
        let identity: String
    }

    static var echo: Echo? {
        ProcessInfo.processInfo.environment["SHEPRD_RELAY_ECHO"].flatMap {
            try? JSONDecoder().decode(Echo.self, from: Data($0.utf8))
        }
    }

    @Test(.enabled(if: echo != nil))
    func bytesCrossTheRealRelayEndToEnd() async throws {
        let echo = try #require(Self.echo)
        let session = try await RelaySession.open(
            endpoint: try #require(RelayEndpoint(echo.endpoint)),
            route: try #require(Base64URL.decode(echo.route)),
            dialKey: P256.Signing.PrivateKey(rawRepresentation: Data(repeating: 0x61, count: 32)),
            tunnelKey: Curve25519.KeyAgreement.PrivateKey(rawRepresentation: Data(repeating: 0x62, count: 32)),
            listener: Tunnel.Identity(bytes: try #require(Base64URL.decode(echo.identity)))
        )
        let sent = Data((0..<50_000).map { UInt8($0 % 251) })
        try await session.send(sent)
        var received = Data()
        while received.count < sent.count, let chunk = try await session.receive() {
            received.append(chunk)
        }
        #expect(received == sent)
        await session.close()
    }

    @Test(.enabled(if: echo != nil))
    func aKeyTheComputerDidNotNameHearsOffline() async throws {
        let echo = try #require(Self.echo)
        await #expect(throws: RelaySession.Failure.refused("offline")) {
            _ = try await RelaySession.open(
                endpoint: try #require(RelayEndpoint(echo.endpoint)),
                route: try #require(Base64URL.decode(echo.route)),
                dialKey: P256.Signing.PrivateKey(),
                tunnelKey: Curve25519.KeyAgreement.PrivateKey(),
                listener: Tunnel.Identity(bytes: try #require(Base64URL.decode(echo.identity)))
            )
        }
    }
}

struct RelayPairingTests {
    struct Pairing: Decodable {
        let secret: String
        let scalar: String
        let `public`: String
    }

    @Test func thePairingDialKeyIsTheOneTheComputerOpens() throws {
        let url = try #require(Bundle.module.url(forResource: "Fixtures/relay/pairing", withExtension: "json"))
        let vector = try JSONDecoder().decode(Pairing.self, from: Data(contentsOf: url))
        let key = RelayProof.pairingDialKey(secret: try TunnelVectorTests.hex(vector.secret))
        #expect(key.rawRepresentation == (try TunnelVectorTests.hex(vector.scalar)))
        #expect(key.publicKey.compressedRepresentation == (try TunnelVectorTests.hex(vector.public)))
    }

    @Test func anOfferWithAndWithoutRelaysReads() throws {
        let plain = try JSONDecoder().decode(NearbySAS.Offer.self, from: Data(#"{"token":"t","origins":["https://a:1"]}"#.utf8))
        #expect(plain.relay == nil)
        let relayed = try JSONDecoder().decode(
            NearbySAS.Offer.self,
            from: Data(#"{"token":"t","origins":[],"relay":{"identity":"i","relays":["https://r"],"route":"x"}}"#.utf8)
        )
        #expect(relayed.relay?.relays == ["https://r"])
    }

    @Test func aClaimCarriesRelayKeysOnlyWhenItHasThem() throws {
        let without = try JSONSerialization.jsonObject(with: JSONEncoder().encode(API.ClaimRequest(token: "t", name: "n", publicKey: "k"))) as? [String: Any]
        #expect(without?["relay"] == nil)
        let keys = API.RelayKeys(dial: "d", tunnel: "t")
        let with = try JSONSerialization.jsonObject(with: JSONEncoder().encode(API.ClaimRequest(token: "t", name: "n", publicKey: "k", relay: keys))) as? [String: Any]
        #expect((with?["relay"] as? [String: String]) == ["dial": "d", "tunnel": "t"])
    }
}
