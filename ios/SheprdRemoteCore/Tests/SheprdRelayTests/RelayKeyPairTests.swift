import Foundation
@testable import SheprdRemoteCore
import Testing

/// A record made with keys this phone already sent, as `HostRelay` makes
/// one when another computer carries the keys to its relays.
struct RelayKeyPairTests {
    @Test func aRecordKeepsTheKeysThePhoneSent() throws {
        let keys = RelayKeyPair()
        let record = try #require(RelayRecord.make(from: RelayRecordTests.info(), keys: keys))
        #expect(record.dialKey == keys.dialKey)
        #expect(record.tunnelKey == keys.tunnelKey)
        #expect(record.enrollment == keys.enrollment)
        #expect(keys.enrollment != nil)
        #expect(RelayKeyPair().enrollment != keys.enrollment)
    }
}
