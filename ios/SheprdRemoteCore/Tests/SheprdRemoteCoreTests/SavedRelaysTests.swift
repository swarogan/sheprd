import Foundation
import Testing
@testable import SheprdRemoteCore

struct SavedRelaysTests {
    private func endpoint(_ text: String) -> RelayEndpoint { RelayEndpoint(text)! }

    @Test func anAddressTypedIsKeptOnceHoweverItIsSpelled() {
        var saved = SavedRelays()
        let result1 = saved.add(" relay.example.org ")
        #expect(result1 == .added(endpoint("https://relay.example.org")))
        let result2 = saved.add("https://relay.example.org/")
        #expect(result2 == .known)
        let result3 = saved.add("wss://RELAY.example.org:443")
        #expect(result3 == .known)
        #expect(saved.relays.map(\.description) == ["https://relay.example.org"])
    }

    @Test func whatIsNotARelayAddressIsRefused() {
        var saved = SavedRelays()
        // Nothing typed, another scheme, a password, plain HTTP in public.
        for text in ["", "   ", "ftp://relay.example.org", "https://me:secret@relay.example.org", "http://relay.example.org"] {
            let result4 = saved.add(text)
            #expect(result4 == .invalid, "\(text)")
        }
        #expect(saved.relays.isEmpty)
    }

    @Test func aComputersRelaysAreKeptAndOnlyNewOnesCount() {
        var saved = SavedRelays([endpoint("https://a.example.org")])
        let named = [endpoint("https://a.example.org"), endpoint("https://b.example.org")]
        let result5 = saved.learn(named)
        #expect(result5)
        let result6 = saved.learn(named)
        #expect(!result6)
        #expect(saved.relays.map(\.description) == ["https://a.example.org", "https://b.example.org"])
        saved.remove(endpoint("https://a.example.org"))
        #expect(saved.relays.map(\.description) == ["https://b.example.org"])
    }

    @Test func noMoreThanTheLimitAreKept() {
        var saved = SavedRelays()
        for index in 0..<SavedRelays.limit {
            let result7 = saved.add("https://r\(index).example.org")
            #expect(result7 != .full)
        }
        let result8 = saved.add("https://one-more.example.org")
        #expect(result8 == .full)
        let result9 = saved.learn([endpoint("https://one-more.example.org")])
        #expect(!result9)
        #expect(saved.relays.count == SavedRelays.limit)
    }

    @Test func theListSurvivesBeingSavedAndAnAddressItCannotRead() throws {
        var saved = SavedRelays()
        _ = saved.add("https://relay.example.org/sheprd")
        _ = saved.add("http://192.168.1.20:8080")
        let data = try JSONEncoder().encode(saved)
        #expect(try JSONDecoder().decode(SavedRelays.self, from: data) == saved)
        let mixed = Data(#"["https://relay.example.org","gopher://old","https://relay.example.org/"]"#.utf8)
        let read = try JSONDecoder().decode(SavedRelays.self, from: mixed)
        #expect(read.relays.map(\.description) == ["https://relay.example.org"])
    }

    @Test func aComputerIsDialedOnItsOwnRelaysThenTheSavedOnes() throws {
        let own = endpoint("https://own.example.org")
        let record = RelayRecord(relays: [own], route: Data(count: 32), identity: Data(), dialKey: Data(), tunnelKey: Data())
        let saved = [endpoint("https://typed.example.org"), own]
        #expect(record.candidates(saved: saved).map(\.description) == ["https://own.example.org", "https://typed.example.org"])
        #expect(record.candidates(saved: []) == [own])
    }

    @Test func aRelaySaysWhatItIsAtItsInfoAddress() {
        #expect(endpoint("relay.example.org/sheprd/").info?.absoluteString == "https://relay.example.org/sheprd/v1/info")
        let info = Data(#"{"protocol":1,"version":"0.1","listen_token":false,"quic":{"port":443,"pin":"x"},"websocket":true}"#.utf8)
        #expect(RelayReach.read(status: 200, body: info) == .reachable)
        // A relay from before the field serves WebSockets.
        #expect(RelayReach.read(status: 200, body: Data(#"{"protocol":1}"#.utf8)) == .reachable)
        #expect(RelayReach.read(status: 200, body: Data(#"{"protocol":1,"websocket":false}"#.utf8)) == .noWebSocket)
        #expect(RelayReach.read(status: 200, body: Data(#"{"protocol":2,"websocket":true}"#.utf8)) == .notARelay)
        #expect(RelayReach.read(status: 200, body: Data("<html>".utf8)) == .notARelay)
        #expect(RelayReach.read(status: 404, body: info) == .notARelay)
    }

    @Test func anAnswerLongerThanARelaysIsNotReadToItsEnd() async throws {
        let short = Endless(length: 8)
        #expect(try await RelayReach.bounded(short, limit: 8) == Data(repeating: 0x7B, count: 8))
        // An answer without end: one byte past the limit is the last read.
        let endless = Endless(length: .max)
        #expect(try await RelayReach.bounded(endless, limit: RelayReach.maxInfo) == nil)
        #expect(endless.pulled.count == RelayReach.maxInfo + 1)
    }
}

/// An answer of `length` bytes that counts how many were asked for.
private struct Endless: AsyncSequence {
    typealias Element = UInt8

    final class Count: @unchecked Sendable {
        var count = 0
    }

    let length: Int
    let pulled = Count()

    struct Iterator: AsyncIteratorProtocol {
        let length: Int
        let pulled: Count

        mutating func next() async -> UInt8? {
            guard pulled.count < length else { return nil }
            pulled.count += 1
            return 0x7B
        }
    }

    func makeAsyncIterator() -> Iterator {
        Iterator(length: length, pulled: pulled)
    }
}
