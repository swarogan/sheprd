import Foundation
@testable import SheprdRelay
@testable import SheprdRemoteCore
import Testing

/// What the computer's owner calls its relays reaches the phone with
/// `GET /api/relay` and is kept with the computer's relay record.
struct RelayLabelTests {
    static let fra3 = RelayLabel(name: "fra3", description: "not for public use", tags: ["eu", "private"])

    static func info(labels: [String: RelayLabel]?) throws -> API.RelayInfo {
        let plain = try RelayRecordTests.info()
        return API.RelayInfo(
            identity: plain.identity,
            relays: ["https://161-35-226-62.sslip.io", "https://fra3.example.org"],
            route: plain.route,
            labels: labels
        )
    }

    @Test func theComputersAnswerReadsWithAndWithoutLabels() throws {
        let labelled = Data("""
        {"identity": "i", "relays": ["https://fra3.example.org"], "route": "r",
         "labels": {"https://fra3.example.org": {"name": "fra3", "description": "not for public use", "tags": ["eu", "private"]},
                    "https://ams3.example.org": {"name": "ams3"}}}
        """.utf8)
        let info = try JSONDecoder().decode(API.RelayInfo.self, from: labelled)
        #expect(info.labels?["https://fra3.example.org"] == Self.fra3)
        // A label without tags leaves the field out.
        #expect(info.labels?["https://ams3.example.org"] == RelayLabel(name: "ams3"))
        let older = Data(#"{"identity": "i", "relays": ["https://fra3.example.org"], "route": "r"}"#.utf8)
        #expect(try JSONDecoder().decode(API.RelayInfo.self, from: older).labels == nil)
    }

    @Test func aRecordKeepsTheLabelsOfTheRelaysItNames() throws {
        // Written without its scheme, and naming a relay not listed.
        let info = try Self.info(labels: ["fra3.example.org/": Self.fra3, "https://gone.example.org": RelayLabel(name: "gone")])
        let record = try #require(RelayRecord.make(from: info))
        let fra3 = try #require(RelayEndpoint("https://fra3.example.org"))
        let sslip = try #require(RelayEndpoint("https://161-35-226-62.sslip.io"))
        #expect(record.label(for: fra3) == Self.fra3)
        #expect(record.label(for: sslip) == nil)
        #expect(record.labels.count == 1)
    }

    @Test func newLabelsReplaceTheOldWithoutNewKeys() throws {
        let record = try #require(RelayRecord.make(from: try Self.info(labels: nil)))
        let renamed = try Self.info(labels: ["https://fra3.example.org": Self.fra3])
        #expect(record.matches(renamed))
        let relabeled = try #require(record.refreshed(with: renamed))
        #expect(relabeled.labels["https://fra3.example.org"] == Self.fra3)
        #expect(relabeled.dialKey == record.dialKey)
        #expect(relabeled.tunnelKey == record.tunnelKey)
        #expect(relabeled.refreshed(with: renamed) == nil)
    }

    @Test func recordsSavedBeforeLabelsReadWithNone() throws {
        let record = try #require(RelayRecord.make(from: try Self.info(labels: ["https://fra3.example.org": Self.fra3])))
        let saved = try JSONEncoder().encode(record)
        #expect(try JSONDecoder().decode(RelayRecord.self, from: saved) == record)
        var old = try #require(try JSONSerialization.jsonObject(with: saved) as? [String: Any])
        old["labels"] = nil
        let older = try JSONSerialization.data(withJSONObject: old)
        let read = try JSONDecoder().decode(RelayRecord.self, from: older)
        #expect(read.labels.isEmpty)
        #expect(read.dialKey == record.dialKey)
        // The computer's round trips, saved beside the labels, are kept.
        var timed = record
        timed.listenerRtt = ["https://fra3.example.org": 12]
        #expect(try JSONDecoder().decode(RelayRecord.self, from: JSONEncoder().encode(timed)) == timed)
    }
}
