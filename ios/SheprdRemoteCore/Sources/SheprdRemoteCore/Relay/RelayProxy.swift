import Foundation
import Network

/// Lets the app's own HTTP and WebSocket code reach a computer through a
/// relay unchanged: a listener on this device's loopback interface, where
/// each connection the app opens becomes one `RelaySession` to the
/// computer, bytes carried both ways. Everything that leaves the device is
/// inside the tunnel.
///
/// Another process on this device can connect to a loopback port too; it
/// then reaches the computer's TLS, where everything but its open pairing
/// and ping routes still needs the phone's bearer token. It could hold
/// sessions, so the proxy carries at most `maxSessions` at once, the
/// relay's limit per dial key, and refuses the rest.
public final class RelayProxy: @unchecked Sendable {
    public typealias Open = @Sendable () async throws -> RelaySession
    typealias OpenCarried = @Sendable () async throws -> any RelayCarried

    /// Sessions at once: the relay allows four per dial key.
    public static let maxSessions = 4

    private let listener: NWListener
    private let open: OpenCarried
    private let queue = DispatchQueue(label: "sheprd.relay.proxy")
    /// The connections carried now, at most `maxSessions`; touched on
    /// `queue` only.
    private var connections: [ObjectIdentifier: NWConnection] = [:]
    /// Set by `stop`, on `queue`: nothing is carried after.
    private var stopped = false

    /// A proxy that opens a session with `open` for each connection.
    public convenience init(open: @escaping Open) throws {
        try self.init(carrying: { try await open() })
    }

    init(carrying open: @escaping OpenCarried) throws {
        let parameters = NWParameters.tcp
        parameters.requiredInterfaceType = .loopback
        parameters.requiredLocalEndpoint = NWEndpoint.hostPort(host: "127.0.0.1", port: .any)
        listener = try NWListener(using: parameters)
        self.open = open
    }

    /// Starts listening; the port to send the app's requests to, once
    /// ready.
    public func start() async throws -> UInt16 {
        try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<UInt16, Error>) in
            let once = Once()
            listener.stateUpdateHandler = { [listener] state in
                switch state {
                case .ready:
                    if once.claim(), let port = listener.port?.rawValue { continuation.resume(returning: port) }
                case let .failed(error):
                    if once.claim() { continuation.resume(throwing: error) }
                default:
                    break
                }
            }
            listener.newConnectionHandler = { [weak self] connection in
                self?.serve(connection)
            }
            listener.start(queue: queue)
        }
    }

    /// Stops listening and ends every session carried: cancelling a
    /// connection ends its two directions, which closes its session.
    public func stop() {
        listener.cancel()
        queue.async {
            self.stopped = true
            for connection in self.connections.values { connection.cancel() }
        }
    }

    /// Sessions carried now.
    public func sessions() async -> Int {
        await withCheckedContinuation { continuation in
            queue.async { continuation.resume(returning: self.connections.count) }
        }
    }

    /// Runs on `queue`, as the listener's handler does.
    private func serve(_ connection: NWConnection) {
        guard !stopped, connections.count < Self.maxSessions else {
            connection.cancel()
            return
        }
        let id = ObjectIdentifier(connection)
        connections[id] = connection
        connection.start(queue: queue)
        let open = open
        Task {
            defer { queue.async { self.connections[id] = nil } }
            guard let session = try? await open() else {
                connection.cancel()
                return
            }
            await Self.carry(connection, session)
        }
    }

    /// Carries bytes both ways until either direction ends, then ends the
    /// other. Cancelling a task does not wake a direction waiting on the
    /// connection or the socket, so each is ended at its source: a
    /// cancelled connection completes its pending receive, and a closed
    /// session fails its pending one. Only then can both be awaited.
    private static func carry(_ connection: NWConnection, _ session: any RelayCarried) async {
        await withTaskGroup(of: Void.self) { group in
            group.addTask { await upstream(connection, session) }
            group.addTask { await downstream(session, connection) }
            await group.next()
            connection.cancel()
            await session.close()
        }
    }

    /// What the app writes goes to the computer.
    private static func upstream(_ connection: NWConnection, _ session: any RelayCarried) async {
        while let data = await receive(connection) {
            if (try? await session.send(data)) == nil { return }
        }
    }

    /// What the computer sends goes to the app.
    private static func downstream(_ session: any RelayCarried, _ connection: NWConnection) async {
        while let data = try? await session.receive() {
            let sent: Bool = await withCheckedContinuation { continuation in
                connection.send(content: data, completion: .contentProcessed { error in
                    continuation.resume(returning: error == nil)
                })
            }
            if !sent { return }
        }
    }

    private static func receive(_ connection: NWConnection) async -> Data? {
        await withCheckedContinuation { continuation in
            connection.receive(minimumIncompleteLength: 1, maximumLength: Tunnel.Transport.maxData) { data, _, complete, error in
                if let data, !data.isEmpty {
                    continuation.resume(returning: data)
                } else if complete || error != nil {
                    continuation.resume(returning: nil)
                } else {
                    continuation.resume(returning: Data())
                }
            }
        }
    }
}

/// What the proxy carries a connection over: a `RelaySession`, or a test's
/// stand-in for one, which needs no relay.
protocol RelayCarried: Sendable {
    func send(_ data: Data) async throws
    func receive() async throws -> Data?
    func close() async
}

extension RelaySession: RelayCarried {}

/// Resumes a continuation once, whatever the listener reports after.
private final class Once: @unchecked Sendable {
    private var done = false
    private let lock = NSLock()

    func claim() -> Bool {
        lock.lock()
        defer { lock.unlock() }
        if done { return false }
        done = true
        return true
    }
}
