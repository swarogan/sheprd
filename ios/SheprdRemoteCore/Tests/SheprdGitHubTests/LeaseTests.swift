import Foundation
import Testing
@testable import SheprdGitHub
@testable import SheprdRemoteCore

struct LeaseTests {
    private let host = GitHubFixtures.host
    private let clock = TestClock()
    private let store = TokenStore(vault: .memory())
    private let lenders = FakeLenders()

    private func leases() -> GitHubLeases {
        GitHubLeases(store: store, now: clock.read)
    }

    @Test func theMostRecentlyReachedLenderIsAskedFirst() {
        let ordered = GitHubLender.order([
            GitHubFixtures.lender("old", reached: -600),
            GitHubFixtures.lender("never", reached: nil),
            GitHubFixtures.lender("other", reached: 0, hosts: ["acme.ghe.com"]),
            GitHubFixtures.lender("new", reached: 0),
        ], for: host)
        #expect(ordered.map(\.device) == ["new", "old", "never"])
    }

    @Test func aFreshLeaseIsKeptAndRecordsItsLender() async throws {
        lenders.answer("mac", GitHubFixtures.answer("t1", expiresIn: 8 * 3600))
        let leases = leases()
        let first = try await leases.lease(for: host, lenders: [GitHubFixtures.lender("mac")], fetch: lenders.fetch)
        let second = try await leases.lease(for: host, lenders: [GitHubFixtures.lender("mac")], fetch: lenders.fetch)
        #expect(first == second)
        #expect(lenders.asked == ["mac"])
        let kept = try #require(store.lease(for: host))
        #expect(kept.lender == "mac")
        #expect(kept.lenderName == "Mac")
        #expect(kept.login == "ana")
        #expect(kept.source == .sheprd)
        #expect(kept.expiresAt == GitHubFixtures.start.addingTimeInterval(8 * 3600))
    }

    @Test func aLeaseNearItsExpiryIsAskedForAgain() async throws {
        lenders.answer("mac", GitHubFixtures.answer("t1", expiresIn: 3600), GitHubFixtures.answer("t2", expiresIn: 4 * 3600))
        let leases = leases()
        let mac = [GitHubFixtures.lender("mac")]
        _ = try await leases.lease(for: host, lenders: mac, fetch: lenders.fetch)
        clock.advance(3600 - GitHubLease.renewal + 1)
        let renewed = try await leases.lease(for: host, lenders: mac, fetch: lenders.fetch)
        #expect(renewed.token == GitHubToken("t2"))
    }

    @Test func aTokenThatDoesNotExpireIsNeverRenewedForAge() async throws {
        lenders.answer("mac", GitHubFixtures.answer("gho", source: .gh))
        let leases = leases()
        _ = try await leases.lease(for: host, lenders: [GitHubFixtures.lender("mac")], fetch: lenders.fetch)
        clock.advance(400 * 86400)
        #expect(try await leases.lease(for: host, lenders: [GitHubFixtures.lender("mac")], fetch: lenders.fetch).token == GitHubToken("gho"))
        #expect(lenders.asked.count == 1)
    }

    @Test func aRefusedTokenIsReplacedByANewLease() async throws {
        lenders.answer("mac", GitHubFixtures.answer("t1", expiresIn: 8 * 3600), GitHubFixtures.answer("t2", expiresIn: 8 * 3600))
        let leases = leases()
        let mac = [GitHubFixtures.lender("mac")]
        let first = try await leases.lease(for: host, lenders: mac, fetch: lenders.fetch)
        await leases.rejected(first)
        #expect(try await leases.lease(for: host, lenders: mac, fetch: lenders.fetch).token == GitHubToken("t2"))
    }

    @Test func aComputerThatStoppedLendingGivesWayToTheNext() async throws {
        lenders.answer("new", .status(403))
        lenders.answer("old", GitHubFixtures.answer("t-old"))
        let lease = try await leases().lease(
            for: host, lenders: [GitHubFixtures.lender("new"), GitHubFixtures.lender("old", reached: -60)], fetch: lenders.fetch
        )
        #expect(lease.lender == "old")
        #expect(lenders.asked == ["new", "old"])
    }

    @Test func everyRefusalIsRefusedAndNoLenderIsNoLender() async throws {
        lenders.answer("mac", .status(404))
        let leases = leases()
        await #expect(throws: LeaseError.refused) {
            try await leases.lease(for: host, lenders: [GitHubFixtures.lender("mac")], fetch: lenders.fetch)
        }
        await #expect(throws: LeaseError.noLender) {
            try await leases.lease(for: host, lenders: [GitHubFixtures.lender("mac", hosts: [])], fetch: lenders.fetch)
        }
        await #expect(throws: LeaseError.unreachable) {
            try await leases.lease(for: host, lenders: [GitHubFixtures.lender("away")], fetch: lenders.fetch)
        }
    }

    @Test func aRateLimitedOrSilentComputerIsLeftAloneForAWhileThatGrows() async throws {
        lenders.answer("mac", .status(429), .status(504), GitHubFixtures.answer("t1"))
        let leases = leases()
        let mac = [GitHubFixtures.lender("mac")]
        await #expect(throws: LeaseError.backingOff(until: GitHubFixtures.start.addingTimeInterval(60))) {
            try await leases.lease(for: host, lenders: mac, fetch: lenders.fetch)
        }
        clock.advance(30)
        await #expect(throws: LeaseError.backingOff(until: GitHubFixtures.start.addingTimeInterval(60))) {
            try await leases.lease(for: host, lenders: mac, fetch: lenders.fetch)
        }
        #expect(lenders.asked.count == 1)
        clock.advance(30)
        // The second wait is twice the first.
        await #expect(throws: LeaseError.backingOff(until: GitHubFixtures.start.addingTimeInterval(60 + 60))) {
            try await leases.lease(for: host, lenders: mac, fetch: lenders.fetch)
        }
        clock.advance(60)
        #expect(try await leases.lease(for: host, lenders: mac, fetch: lenders.fetch).token == GitHubToken("t1"))
    }

    @Test func aStillValidLeaseServesWhileNoComputerAnswers() async throws {
        lenders.answer("mac", GitHubFixtures.answer("t1", expiresIn: 600))
        let leases = leases()
        let mac = [GitHubFixtures.lender("mac")]
        _ = try await leases.lease(for: host, lenders: mac, fetch: lenders.fetch)
        clock.advance(400)
        // Near its expiry, and the computer is away: the old one still works.
        #expect(try await leases.lease(for: host, lenders: mac, fetch: lenders.fetch).token == GitHubToken("t1"))
        clock.advance(300)
        await #expect(throws: LeaseError.unreachable) { try await leases.lease(for: host, lenders: mac, fetch: lenders.fetch) }
    }

    @Test func aLeaseForAnotherHostIsNotKept() async throws {
        lenders.answer("mac", GitHubFixtures.answer("t1", host: "evil.example.com"))
        await #expect(throws: LeaseError.unreachable) {
            try await leases().lease(for: host, lenders: [GitHubFixtures.lender("mac")], fetch: lenders.fetch)
        }
        #expect(store.lease(for: host) == nil)
    }

    @Test func leasesGoWithTheirComputerOrItsSharing() async throws {
        lenders.answer("mac", GitHubFixtures.answer("t1"))
        let leases = leases()
        _ = try await leases.lease(for: host, lenders: [GitHubFixtures.lender("mac")], fetch: lenders.fetch)
        // A computer whose home has not arrived yet keeps what it lent.
        await leases.reconcile([GitHubLender(device: "mac", name: "Mac", hosts: nil, reached: nil)])
        #expect(store.lease(for: host) != nil)
        // Its home no longer lends the host.
        await leases.reconcile([GitHubFixtures.lender("mac", hosts: [])])
        #expect(store.lease(for: host) == nil)

        lenders.answer("mac", GitHubFixtures.answer("t2"))
        _ = try await leases.lease(for: host, lenders: [GitHubFixtures.lender("mac")], fetch: lenders.fetch)
        // The computer was forgotten.
        await leases.reconcile([])
        #expect(store.lease(for: host) == nil)

        lenders.answer("mac", GitHubFixtures.answer("t3"))
        _ = try await leases.lease(for: host, lenders: [GitHubFixtures.lender("mac")], fetch: lenders.fetch)
        await leases.forget(lender: "mac")
        #expect(await leases.all().isEmpty)
    }

    @Test func aTokenNeverReachesADescription() async throws {
        let token = GitHubToken("ghu_secret")
        let lease = GitHubLease(host: host, login: "ana", token: token, expiresAt: nil, source: .sheprd, lender: "mac", lenderName: "Mac", received: GitHubFixtures.start)
        for text in [String(describing: token), String(reflecting: token), String(describing: lease), String(reflecting: lease), "\(LeaseAnswer(host: "github.com", login: "ana", token: token, source: .gh))"] {
            #expect(!text.contains("ghu_secret"))
        }
        var dumped = ""
        dump(lease, to: &dumped)
        #expect(!dumped.contains("ghu_secret"))
    }

    @Test func theWireAnswerDecodes() throws {
        let json = #"{"host":"github.com","login":"ana","token":"ghu_x","expires_at":1800000000,"source":"sheprd"}"#
        let answer = try JSONDecoder().decode(LeaseAnswer.self, from: Data(json.utf8))
        #expect(answer.expiresAt == 1_800_000_000)
        #expect(answer.source == .sheprd)
        let gh = try JSONDecoder().decode(LeaseAnswer.self, from: Data(#"{"host":"x.ghe.com","login":"a","token":"t","source":"future"}"#.utf8))
        #expect(gh.expiresAt == nil)
        #expect(gh.source == .other)
    }
}
