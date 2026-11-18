import Foundation
import SheprdRemoteCore
import Testing

@Suite struct ComputerReachTests {
    private func computer(origins: [String], relay: RelayRecord? = nil) throws -> Computer {
        Computer(
            device: "d_1", name: "Studio", origins: origins,
            fingerprint: try #require(Fingerprint(String(repeating: "AB", count: 32))),
            keyKind: .software, keyData: Data(), relay: relay
        )
    }

    @Test func aComputerWithoutRelaysShowsItsAddressAlone() throws {
        let tailscale = try computer(origins: ["https://127.0.0.1:7443", "https://100.101.102.103:7443"])
        #expect(tailscale.reachLabel == "Tailscale")
        #expect(tailscale.relayHosts.isEmpty)
        #expect(try computer(origins: []).reachLabel == nil)
    }

    @Test func aRelayedComputerSaysSoBesideItsAddress() throws {
        let record = RelayRecord(
            relays: [try #require(RelayEndpoint("https://161-35-226-62.sslip.io"))],
            route: Data(repeating: 0x52, count: 32), identity: Data(), dialKey: Data(), tunnelKey: Data()
        )
        let relayed = try computer(origins: ["https://100.101.102.103:7443"], relay: record)
        #expect(relayed.reachLabel == "Tailscale · Relay")
        #expect(relayed.relayHosts == ["161-35-226-62.sslip.io"])
        // Reached only through relays, it still says how.
        #expect(try computer(origins: [], relay: record).reachLabel == "Relay")
    }
}
