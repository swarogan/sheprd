import Foundation
import Network
import Testing
@testable import SheprdRemoteCore

/// A session whose `receive` waits, as a WebSocket's does, until the relay
/// drops it or it is closed.
private actor WaitingSession: RelayCarried {
    private var waiting: CheckedContinuation<Data?, Never>?
    private var ended = false
    private(set) var closed = false

    func send(_: Data) async throws {}

    func receive() async throws -> Data? {
        if ended { return nil }
        return await withCheckedContinuation { waiting = $0 }
    }

    /// The relay went away: the pending `receive` ends.
    func drop() {
        end()
    }

    func close() async {
        closed = true
        end()
    }

    private func end() {
        ended = true
        waiting?.resume(returning: nil)
        waiting = nil
    }
}

/// Polls `condition` for up to five seconds.
private func eventually(_ condition: @Sendable () async -> Bool) async -> Bool {
    for _ in 0..<500 {
        if await condition() { return true }
        try? await Task.sleep(for: .milliseconds(10))
    }
    return false
}

/// An app connection to the proxy, open and idle: it sends nothing, so
/// the proxy's read of it stays pending.
private func connect(port: UInt16) async -> NWConnection {
    let connection = NWConnection(
        host: "127.0.0.1", port: NWEndpoint.Port(rawValue: port)!, using: .tcp
    )
    await withCheckedContinuation { (continuation: CheckedContinuation<Void, Never>) in
        connection.stateUpdateHandler = { state in
            if case .ready = state {
                connection.stateUpdateHandler = nil
                continuation.resume()
            }
        }
        connection.start(queue: .global())
    }
    return connection
}

/// Whether the proxy closed `connection`: its next read ends.
private func closed(_ connection: NWConnection) async -> Bool {
    await withCheckedContinuation { continuation in
        connection.receive(minimumIncompleteLength: 1, maximumLength: 16) { data, _, complete, error in
            continuation.resume(returning: data == nil && (complete || error != nil))
        }
    }
}

@Suite(.timeLimit(.minutes(1)))
struct RelayProxyTests {
    @Test func aDroppedRelayEndsTheConnectionAndFreesItsSlot() async throws {
        let session = WaitingSession()
        let proxy = try RelayProxy(carrying: { session })
        let port = try await proxy.start()
        defer { proxy.stop() }
        let app = await connect(port: port)
        #expect(await eventually { await proxy.sessions() == 1 })

        // The relay drops while the proxy waits on the app's next bytes.
        await session.drop()

        #expect(await closed(app))
        #expect(await eventually { await proxy.sessions() == 0 })
        #expect(await session.closed)
    }

    @Test func stoppingClosesTheConnectionsCarried() async throws {
        let session = WaitingSession()
        let proxy = try RelayProxy(carrying: { session })
        let port = try await proxy.start()
        let app = await connect(port: port)
        #expect(await eventually { await proxy.sessions() == 1 })

        proxy.stop()

        #expect(await closed(app))
        #expect(await eventually { await session.closed })
        #expect(await eventually { await proxy.sessions() == 0 })
    }

    @Test func noMoreThanTheRelaysLimitAreCarriedAtOnce() async throws {
        let proxy = try RelayProxy(carrying: { WaitingSession() })
        let port = try await proxy.start()
        defer { proxy.stop() }
        var apps: [NWConnection] = []
        for _ in 0..<RelayProxy.maxSessions { apps.append(await connect(port: port)) }
        #expect(await eventually { await proxy.sessions() == RelayProxy.maxSessions })
        let refused = await connect(port: port)
        #expect(await closed(refused))
        #expect(await proxy.sessions() == RelayProxy.maxSessions)
        for app in apps { app.cancel() }
        #expect(await eventually { await proxy.sessions() == 0 })
    }
}
