import Foundation
@testable import SheprdRelay
@testable import SheprdRemoteCore
import Testing

/// Choosing among a computer's relays by the whole path: this phone's leg
/// plus the computer's.
struct RelayNearnessTests {
    static let europe = RelayEndpoint("https://europe.example.org")!
    static let asia = RelayEndpoint("https://asia.example.org")!
    static let usWest = RelayEndpoint("https://us-west.example.org")!
    static let usEast = RelayEndpoint("https://us-east.example.org")!

    @Test func thePathIsBothLegsNotTheRelayNearestThePhone() {
        // A computer in Europe, a phone in Asia: Asia's relay is nearest
        // the phone, but through Europe's the whole path is shorter.
        let relays = [Self.europe, Self.usEast, Self.asia, Self.usWest]
        let listener = [
            Self.europe.description: 10, Self.usEast.description: 90,
            Self.asia.description: 250, Self.usWest.description: 150,
        ]
        let phone = [Self.europe: 220, Self.usEast: 200, Self.asia: 15, Self.usWest: 120]
        #expect(RelayNearness.ranked(relays, listener: listener, phone: phone) == [Self.europe, Self.asia, Self.usWest, Self.usEast])
    }

    @Test func withoutTheComputersTimesThePhonesAloneDecide() {
        let relays = [Self.europe, Self.asia]
        #expect(RelayNearness.ranked(relays, listener: [:], phone: [Self.europe: 200, Self.asia: 20]) == [Self.asia, Self.europe])
        // Nothing measured at all: the computer's order.
        #expect(RelayNearness.ranked(relays, listener: [:], phone: [:]) == relays)
    }

    @Test func aRelayTheComputerIsNotOnOrThePhoneCannotReachGoesLast() {
        let relays = [Self.europe, Self.asia, Self.usEast]
        // The computer reports no time for Asia: it is not listening there.
        let listener = [Self.europe.description: 300, Self.usEast.description: 300]
        let phone = [Self.europe: 300, Self.asia: 1, Self.usEast: 1]
        #expect(RelayNearness.ranked(relays, listener: listener, phone: phone) == [Self.usEast, Self.europe, Self.asia])
        // The phone could not probe Europe: after those it reached.
        let unprobed = [Self.asia: 1, Self.usEast: 500]
        #expect(RelayNearness.ranked(relays, listener: listener, phone: unprobed).first == Self.usEast)
    }

    @Test func probesAreFreshForAWhileAndForgottenOnANewNetwork() async {
        let probes = RelayProbes()
        let start = Date(timeIntervalSince1970: 1_000)
        await probes.record(Self.europe, millis: 0, at: start)
        #expect(await probes.rtt([Self.europe, Self.asia], now: start) == [Self.europe: 1])
        #expect(await probes.rtt([Self.europe], now: start + RelayProbes.fresh + 1).isEmpty)
        await probes.record(Self.europe, millis: 40, at: start)
        await probes.forget()
        #expect(await probes.rtt([Self.europe], now: start).isEmpty)
    }

    @Test func aProbeFromTheOldNetworkIsNotRecordedAfterAChange() async {
        let probes = RelayProbes()
        let old = await probes.currentNetwork
        await probes.forget()
        await probes.finished(Self.europe, millis: 40, network: old)
        #expect(await probes.rtt([Self.europe]).isEmpty)
        await probes.finished(Self.europe, millis: 40, network: probes.currentNetwork)
        #expect(await probes.rtt([Self.europe]) == [Self.europe: 40])
    }

    @Test func theComputersNewOrderIsSavedButJitterIsNot() throws {
        let base = try RelayRecordTests.info()
        let info = API.RelayInfo(
            identity: base.identity, relays: [Self.europe.description, Self.asia.description], route: base.route,
            rttMs: [Self.europe.description: 20, Self.asia.description: 200]
        )
        let record = try #require(RelayRecord.make(from: info))
        #expect(record.listenerRtt == info.rttMs)
        let jitter = API.RelayInfo(
            identity: info.identity, relays: info.relays, route: info.route,
            rttMs: [Self.europe.description: 35, Self.asia.description: 230]
        )
        #expect(record.matches(jitter))
        #expect(record.refreshed(with: jitter) == nil)
        // The computer moved to Asia: same keys, new order.
        let moved = API.RelayInfo(
            identity: info.identity, relays: [Self.asia.description, Self.europe.description], route: info.route,
            rttMs: [Self.europe.description: 210, Self.asia.description: 12]
        )
        #expect(record.matches(moved))
        let refreshed = try #require(record.refreshed(with: moved))
        #expect(refreshed.relays == [Self.asia, Self.europe])
        #expect(refreshed.dialKey == record.dialKey)
    }

    @Test func theComputersTimesAreReadAndOlderAnswersStillDecode() throws {
        let newer = try JSONDecoder().decode(
            API.RelayInfo.self,
            from: Data(#"{"identity":"i","relays":["https://r"],"route":"x","rtt_ms":{"https://r":12}}"#.utf8)
        )
        #expect(newer.rttMs == ["https://r": 12])
        let older = try JSONDecoder().decode(API.RelayInfo.self, from: Data(#"{"identity":"i","relays":["https://r"],"route":"x"}"#.utf8))
        #expect(older.rttMs == nil)
        // A record saved before round trips were kept still loads.
        let saved = try JSONEncoder().encode(RelayRecord(relays: [Self.europe], route: Data(), identity: Data(), dialKey: Data(), tunnelKey: Data()))
        var object = try #require(JSONSerialization.jsonObject(with: saved) as? [String: Any])
        object["listenerRtt"] = nil
        let loaded = try JSONDecoder().decode(RelayRecord.self, from: JSONSerialization.data(withJSONObject: object))
        #expect(loaded.listenerRtt == nil)
    }
}
