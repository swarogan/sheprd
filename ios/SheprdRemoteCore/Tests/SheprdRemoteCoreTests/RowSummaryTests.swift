import Foundation
@testable import SheprdRemoteCore
import Testing

/// A chat's row summary, as the computer sends it and as long as it holds.
struct RowSummaryTests {
    private func summary(_ extra: String) throws -> Home.Summary {
        let json = #"{"connected":true,"status":{"state":"idle"},"questions":0"# + extra + "}"
        return try JSONDecoder().decode(Home.Summary.self, from: Data(json.utf8))
    }

    @Test func readsEveryState() throws {
        #expect(try summary(#","row_summary":{"state":"pending","since_ms":5}"#).rowSummary == .pending(sinceMs: 5))
        #expect(try summary(#","row_summary":{"state":"ready","text":"Fixed.","at_ms":7}"#).rowSummary == .ready(text: "Fixed.", atMs: 7))
        #expect(try summary(#","row_summary":{"state":"failed","reason":"No agent","at_ms":7}"#).rowSummary == .failed(reason: "No agent", atMs: 7))
        #expect(try summary(#","row_summary":{"state":"queued"}"#).rowSummary == .other)
        #expect(try summary("").rowSummary == nil)
    }

    @Test func holdsUntilANewerTurnEnds() throws {
        let ready = #","row_summary":{"state":"ready","text":"Fixed.","at_ms":100}"#
        #expect(try summary(ready).currentRowSummary == .ready(text: "Fixed.", atMs: 100))
        #expect(try summary(ready + #","last_turn_ended_ms":50"#).currentRowSummary != nil)
        #expect(try summary(ready + #","last_turn_ended_ms":150"#).currentRowSummary == nil)
    }

    @Test func aRunningSummaryAlwaysShows() throws {
        let pending = #","row_summary":{"state":"pending","since_ms":100},"last_turn_ended_ms":150"#
        #expect(try summary(pending).currentRowSummary == .pending(sinceMs: 100))
    }

    @Test func roundTrips() throws {
        for value in [RowSummary.pending(sinceMs: 1), .ready(text: "A", atMs: 2), .failed(reason: "B", atMs: 3)] {
            #expect(try JSONDecoder().decode(RowSummary.self, from: JSONEncoder().encode(value)) == value)
        }
    }
}
