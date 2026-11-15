import Foundation
import Testing
@testable import SheprdRemoteCore

/// Dialing a computer on several relays: a silent one must not hold back
/// the one that works, and only one session is kept.
@Suite struct RelayRaceTests {
    struct Down: Error, Equatable {
        let relay: String
    }

    /// What was started and discarded, across tasks.
    final class Log: @unchecked Sendable {
        private let lock = NSLock()
        private var startedNames: [String] = []
        private var discardedNames: [String] = []

        var started: [String] { lock.withLock { startedNames } }
        var discarded: [String] { lock.withLock { discardedNames } }

        func start(_ name: String) { lock.withLock { startedNames.append(name) } }
        func discard(_ name: String) { lock.withLock { discardedNames.append(name) } }
    }

    /// Races `relays`, each behaving as `open` says, with `stagger` between.
    private func race(
        _ relays: [String], stagger: Duration, log: Log,
        open: @escaping @Sendable (String) async throws -> String
    ) async throws -> String {
        try await RelayRace.first(
            of: relays, stagger: stagger, none: Down(relay: "none"),
            open: { relay in
                log.start(relay)
                return try await open(relay)
            },
            discard: { log.discard($0) }
        )
    }

    @Test func aSilentRelayDoesNotHoldBackTheNext() async throws {
        let log = Log()
        let winner = try await race(["old", "new"], stagger: .milliseconds(20), log: log) { relay in
            // The old relay never answers; cancelling it ends the wait.
            if relay == "old" { try await Task.sleep(for: .seconds(3_600)) }
            return relay
        }
        #expect(winner == "new")
        #expect(log.started == ["old", "new"])
        #expect(log.discarded.isEmpty)
    }

    @Test func aRelayThatFailsGivesWayAtOnce() async throws {
        let log = Log()
        // An hour between starts: only the failure can start the next.
        let winner = try await race(["old", "new"], stagger: .seconds(3_600), log: log) { relay in
            if relay == "old" { throw Down(relay: relay) }
            return relay
        }
        #expect(winner == "new")
    }

    @Test func theFirstRelayToAnswerIsTheOnlyOneDialed() async throws {
        let log = Log()
        let winner = try await race(["own", "saved"], stagger: .seconds(3_600), log: log) { $0 }
        #expect(winner == "own")
        #expect(log.started == ["own"])
    }

    @Test func aSessionOpenedAfterAnotherWonIsClosed() async throws {
        let log = Log()
        let winner = try await race(["slow", "fast"], stagger: .milliseconds(20), log: log) { relay in
            // Opens anyway, as a dial already past its last wait would.
            if relay == "slow" { try? await Task.sleep(for: .milliseconds(200)) }
            return relay
        }
        #expect(winner == "fast")
        #expect(log.discarded == ["slow"])
    }

    @Test func withNoRelayAnsweringTheLastFailureIsThrown() async {
        let log = Log()
        await #expect(throws: Down(relay: "b")) {
            try await race(["a", "b"], stagger: .seconds(3_600), log: log) { throw Down(relay: $0) }
        }
        await #expect(throws: Down(relay: "none")) {
            try await race([], stagger: .seconds(3_600), log: log) { $0 }
        }
        #expect(log.started == ["a", "b"])
    }
}
