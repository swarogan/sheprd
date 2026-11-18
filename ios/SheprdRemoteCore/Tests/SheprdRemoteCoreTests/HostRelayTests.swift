import CryptoKit
import Foundation
import Testing
@testable import SheprdRemoteCore

struct HostRelayTests {
    static let far = Fingerprint(String(repeating: "00112233445566778899AABBCCDDEEFF", count: 2))!

    /// The same bytes `control/tests/host_relay.rs` expects the daemon to
    /// verify.
    @Test func theMessageMatchesTheDaemons() throws {
        let message = try #require(HostRelay.message(
            fingerprint: Self.far,
            device: "dev",
            keys: API.RelayKeys(dial: "DIAL", tunnel: "TUN"),
            signedMs: 0x0102_0304_0506_0708
        ))
        var expected = Data("sheprd-host-relay-v1".utf8) + Self.far.bytes
        expected += Data([0, 3]) + Data("dev".utf8)
        expected += Data([0, 4]) + Data("DIAL".utf8)
        expected += Data([0, 3]) + Data("TUN".utf8)
        expected += Data([1, 2, 3, 4, 5, 6, 7, 8])
        #expect(message == expected)
    }

    @Test func theRequestUsesTheDaemonsFieldNames() throws {
        let request = API.HostRelayRequest(
            fingerprint: "AB", device: "dev", keys: API.RelayKeys(dial: "D", tunnel: "T"), signedMs: 7, signature: "S"
        )
        let object = try #require(JSONSerialization.jsonObject(with: JSONEncoder().encode(request)) as? [String: Any])
        #expect(object["signed_ms"] as? Int == 7)
        #expect((object["keys"] as? [String: String]) == ["dial": "D", "tunnel": "T"])
    }

    @Test func statesDecodeAsTheDaemonSendsThem() throws {
        func state(_ json: String) throws -> API.HostRelayState {
            try JSONDecoder().decode(API.HostRelayState.self, from: Data(json.utf8))
        }
        #expect(try state(#"{"state":"pending"}"#) == .pending)
        let proof = #""proof":{"certificate":"c","key":"k","signature":"s"}"#
        let signed = API.RelayProof(certificate: "c", key: "k", signature: "s")
        #expect(try state(#"{"state":"done","outcome":"kept",\#(proof)}"#) == .kept(signed))
        // A claim without the far computer's signature does not read.
        #expect(throws: DecodingError.self) { try state(#"{"state":"done","outcome":"kept"}"#) }
        #expect(try state(#"{"state":"done","outcome":"elsewhere"}"#) == .elsewhere)
        #expect(try state(#"{"state":"done","outcome":"failed","reason":"no"}"#) == .failed(reason: "no"))
        #expect(
            try state(#"{"state":"done","outcome":"relayed","identity":"i","relays":["https://r.example"],"route":"x",\#(proof)}"#)
                == .relayed(API.RelayInfo(identity: "i", relays: ["https://r.example"], route: "x"), signed)
        )
        #expect(throws: DecodingError.self) { try state(#"{"state":"done","outcome":"later"}"#) }
    }

    static func computer(_ device: String, fingerprint: String, via: String? = nil) -> Computer {
        Computer(
            device: device, name: device, origins: [], fingerprint: Fingerprint(String(repeating: fingerprint, count: 32))!,
            keyKind: .software, keyData: Data(), via: via.map { Fingerprint(String(repeating: $0, count: 32))! }
        )
    }

    @Test func onlyAComputerOutOfReachIsCarriedToThroughAnOnlineOne() {
        var plan = HostRelayPlan()
        let studio = Self.computer("studio", fingerprint: "11")
        let framework = Self.computer("framework", fingerprint: "22")
        let laptop = Self.computer("laptop", fingerprint: "33")
        let now = Date(timeIntervalSince1970: 1_000_000)
        let picks = plan.due([
            (studio, .online(origin: "https://127.0.0.1:1")),
            (framework, .offline(reason: "no route")),
            (laptop, .connecting),
        ], at: now)
        #expect(picks == [HostRelayPlan.Pick(far: framework, near: studio)])
        // Running: not started twice.
        #expect(plan.due([(studio, .online(origin: "o")), (framework, .offline(reason: ""))], at: now).isEmpty)
        // Finished: the same near computer waits before it is asked again.
        plan.finished("framework", at: now)
        #expect(plan.due([(studio, .online(origin: "o")), (framework, .offline(reason: ""))], at: now + 60).isEmpty)
        #expect(
            plan.due([(studio, .online(origin: "o")), (framework, .offline(reason: ""))], at: now + HostRelayPlan.retry)
                .count == 1
        )
    }

    @Test func aSettledAnswerLeavesTheComputerAloneForADay() {
        var plan = HostRelayPlan()
        let studio = Self.computer("studio", fingerprint: "11")
        let m4 = Self.computer("m4", fingerprint: "44")
        let framework = Self.computer("framework", fingerprint: "22")
        let now = Date(timeIntervalSince1970: 1_000_000)
        let machines: [(computer: Computer, status: LinkStatus)] = [
            (studio, .online(origin: "o")), (m4, .online(origin: "o")), (framework, .offline(reason: "")),
        ]
        #expect(plan.due(machines, at: now).count == 1)
        plan.finished("framework", settled: true, at: now)
        // Another near computer is not tried either, until the day is over.
        #expect(plan.due(machines, at: now + HostRelayPlan.retry).isEmpty)
        #expect(plan.due(machines, at: now + HostRelayPlan.settled).count == 1)
        // Reached directly, it starts over.
        plan.finished("framework", settled: true, at: now)
        plan.forget("framework")
        #expect(plan.due(machines, at: now + 1).count == 1)
    }

    @Test func theComputerThatVouchedIsAskedFirstAndOneWithRelaysIsLeftAlone() {
        var plan = HostRelayPlan()
        let studio = Self.computer("studio", fingerprint: "11")
        let m4 = Self.computer("m4", fingerprint: "44")
        let host = Self.computer("framework", fingerprint: "22", via: "44")
        let picks = plan.due([
            (studio, .online(origin: "o")), (m4, .online(origin: "o")), (host, .offline(reason: "")),
        ], at: .now)
        #expect(picks.map(\.near.device) == ["m4"])
        var reached = Self.computer("reached", fingerprint: "55")
        reached.relay = RelayRecord(relays: [], route: Data(), identity: Data(), dialKey: Data(), tunnelKey: Data())
        #expect(plan.due([(studio, .online(origin: "o")), (reached, .offline(reason: ""))], at: .now).isEmpty)
        // With nothing online there is no one to ask.
        var alone = HostRelayPlan()
        #expect(alone.due([(host, .offline(reason: ""))], at: .now).isEmpty)
    }

    /// The same bytes `control/tests/host_relay.rs` expects the daemon to
    /// sign.
    @Test func theAnswerMessageMatchesTheDaemons() throws {
        let request = API.HostRelayRequest(
            fingerprint: Self.far.description, device: "dev", keys: API.RelayKeys(dial: "DIAL", tunnel: "TUN"),
            signedMs: 0x0102_0304_0506_0708, signature: "SIG"
        )
        let signed = try #require(HostRelay.message(
            fingerprint: Self.far, device: "dev", keys: request.keys, signedMs: request.signedMs
        ))
        let kept = try #require(HostRelay.answerMessage(for: request, info: nil))
        #expect(kept == Data("sheprd-host-relay-answer-v1".utf8) + signed + Data([2]))
        let info = API.RelayInfo(identity: "ID", relays: ["https://a", "https://b"], route: "RT")
        let relayed = try #require(HostRelay.answerMessage(for: request, info: info))
        var expected = Data("sheprd-host-relay-answer-v1".utf8) + signed + Data([1])
        expected += Data([0, 2]) + Data("ID".utf8) + Data([0, 2]) + Data("RT".utf8) + Data([0, 2])
        expected += Data([0, 9]) + Data("https://a".utf8) + Data([0, 9]) + Data("https://b".utf8)
        #expect(relayed == expected)
    }

    /// Signs with the CA in the presence fixture, whose key is 32 bytes of
    /// 8 and whose certificate the daemon made.
    @Test func onlyAnAnswerSignedByThePinnedCertificateIsProven() throws {
        let fixture = try PresenceFixtureTests.fixture()
        let pinned = try PresenceFixtureTests.pinned(fixture)
        let ca = try P256.Signing.PrivateKey(rawRepresentation: Data(repeating: 8, count: 32))
        let request = API.HostRelayRequest(
            fingerprint: pinned.description, device: "dev", keys: API.RelayKeys(dial: "D", tunnel: "T"),
            signedMs: 42, signature: "S"
        )
        let info = API.RelayInfo(identity: "ID", relays: ["https://relay.example.org"], route: "RT")
        func proof(_ info: API.RelayInfo?, signer: P256.Signing.PrivateKey, certificate: String) throws -> API.RelayProof {
            let message = try #require(HostRelay.answerMessage(for: request, info: info))
            return API.RelayProof(
                certificate: certificate,
                key: Base64URL.encode(signer.publicKey.x963Representation),
                signature: Base64URL.encode(try signer.signature(for: message).rawRepresentation)
            )
        }
        let good = try proof(info, signer: ca, certificate: fixture.ca_certificate)
        #expect(HostRelay.proven(good, info: info, for: request))
        #expect(HostRelay.proven(try proof(nil, signer: ca, certificate: fixture.ca_certificate), info: nil, for: request))
        // Relays changed after signing, or the answer turned into another.
        let moved = API.RelayInfo(identity: "ID", relays: ["https://elsewhere.example.org"], route: "RT")
        #expect(!HostRelay.proven(good, info: moved, for: request))
        #expect(!HostRelay.proven(good, info: nil, for: request))
        // Another host signing with its own key, the real certificate shown.
        let impostor = P256.Signing.PrivateKey()
        #expect(!HostRelay.proven(try proof(info, signer: impostor, certificate: fixture.ca_certificate), info: info, for: request))
        // A request for another computer.
        let other = API.HostRelayRequest(
            fingerprint: Self.far.description, device: "dev", keys: request.keys, signedMs: 42, signature: "S"
        )
        #expect(!HostRelay.proven(good, info: info, for: other))
    }
}
