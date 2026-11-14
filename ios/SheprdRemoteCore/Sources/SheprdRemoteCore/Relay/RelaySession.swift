import CryptoKit
import Foundation

/// One session to a computer through a relay, over the relay's WebSocket
/// fallback, with the end-to-end tunnel inside: the dial and its proof,
/// then the hybrid Noise handshake against the computer's pinned identity,
/// then records. The relay sees only ciphertext; a relay or anyone between
/// that is not the paired computer cannot finish the handshake.
public actor RelaySession {
    public enum Failure: Error, Equatable {
        /// The relay answered the dial with this status (`offline`, `busy`).
        case refused(String)
        /// The relay or the computer said something that is not the protocol.
        case protocolError
        case closed
    }

    let socket: URLSessionWebSocketTask
    var transport: Tunnel.Transport
    var decoder: Tunnel.Frame.Decoder
    var finished = false

    init(socket: URLSessionWebSocketTask, transport: Tunnel.Transport, decoder: Tunnel.Frame.Decoder) {
        self.socket = socket
        self.transport = transport
        self.decoder = decoder
    }

    /// Which tunnel handshake to run once the relay lets the dial through.
    public enum Handshake: Sendable {
        /// A paired phone's session, against the pinned identity.
        case session(local: Curve25519.KeyAgreement.PrivateKey, listener: Tunnel.Identity)
        /// Pairing, with the PSK from the offer's token.
        case pairing(local: Curve25519.KeyAgreement.PrivateKey, listener: Tunnel.Identity, secret: Data)
    }

    /// Dials `route` on `endpoint` with `dialKey`, then runs the tunnel's
    /// session handshake with `tunnelKey` against `listener`.
    public static func open(
        endpoint: RelayEndpoint,
        route: Data,
        dialKey: P256.Signing.PrivateKey,
        tunnelKey: Curve25519.KeyAgreement.PrivateKey,
        listener: Tunnel.Identity,
        session: URLSession = .shared
    ) async throws -> RelaySession {
        try await open(
            endpoint: endpoint, route: route, dialKey: dialKey,
            tunnel: .session(local: tunnelKey, listener: listener), session: session
        )
    }

    /// Dials `route` on `endpoint` with `dialKey`, then runs `tunnel`.
    public static func open(
        endpoint: RelayEndpoint,
        route: Data,
        dialKey: P256.Signing.PrivateKey,
        tunnel: Handshake,
        session: URLSession = .shared
    ) async throws -> RelaySession {
        guard let url = endpoint.dial(route: route) else { throw Failure.protocolError }
        let socket = session.webSocketTask(with: url)
        socket.maximumMessageSize = 2 * Tunnel.Transport.maxData + 1_024
        socket.resume()
        do {
            guard case let .string(challengeText) = try await socket.receive(),
                  let challenge = try? JSONDecoder().decode(Challenge.self, from: Data(challengeText.utf8)),
                  challenge.t == "challenge", challenge.version == 1,
                  let nonce = Base64URL.decode(challenge.nonce), nonce.count == RelayProof.nonceLength
            else { throw Failure.protocolError }
            let signed = RelayProof.dialSigned(name: endpoint.proofName, nonce: nonce, route: route)
            let hello = try JSONEncoder().encode(RelayProof.hello(key: dialKey, signed: signed))
            try await socket.send(.string(String(decoding: hello, as: UTF8.self)))
            guard case let .string(statusText) = try await socket.receive(),
                  let status = try? JSONDecoder().decode(StatusMessage.self, from: Data(statusText.utf8)),
                  status.t == "status"
            else { throw Failure.protocolError }
            guard status.status == "ok" else { throw Failure.refused(status.status) }

            let (initiator, first): (Tunnel.Initiator, Data)
            switch tunnel {
            case let .session(local, listener):
                (initiator, first) = try Tunnel.Initiator.session(local: local, listener: listener, route: route)
            case let .pairing(local, listener, secret):
                (initiator, first) = try Tunnel.Initiator.pairing(
                    local: local, listenerX25519: listener.x25519,
                    listenerFingerprint: listener.fingerprint, route: route, secret: secret
                )
            }
            try await socket.send(.data(Tunnel.Frame.encode(first)))
            var decoder = Tunnel.Frame.Decoder()
            let answer = try await nextFrame(socket: socket, decoder: &decoder)
            let (transport, _) = try initiator.finish(answer: answer)
            return RelaySession(socket: socket, transport: transport, decoder: decoder)
        } catch {
            socket.cancel(with: .goingAway, reason: nil)
            throw error
        }
    }

    struct Challenge: Decodable {
        let t: String
        let nonce: String
        let version: Int
    }

    struct StatusMessage: Decodable {
        let t: String
        let status: String
    }

    static func nextFrame(socket: URLSessionWebSocketTask, decoder: inout Tunnel.Frame.Decoder) async throws -> Data {
        while true {
            if let frame = decoder.next() { return frame }
            switch try await socket.receive() {
            case let .data(bytes): decoder.push(bytes)
            case .string: throw Failure.protocolError
            @unknown default: throw Failure.protocolError
            }
        }
    }

    /// The next whole frame, reading the socket as needed; on the actor,
    /// so the decoder is never handed across an `await`.
    func nextFrame() async throws -> Data {
        while true {
            if let frame = decoder.next() { return frame }
            switch try await socket.receive() {
            case let .data(bytes): decoder.push(bytes)
            case .string: throw Failure.protocolError
            @unknown default: throw Failure.protocolError
            }
        }
    }

    /// Sends `data` to the computer, in as many records as it takes.
    public func send(_ data: Data) async throws {
        guard !finished else { throw Failure.closed }
        var offset = data.startIndex
        while offset < data.endIndex {
            let end = min(offset + Tunnel.Transport.maxData, data.endIndex)
            let record = try transport.seal(Data(data[offset..<end]))
            try await socket.send(.data(Tunnel.Frame.encode(record)))
            offset = end
        }
    }

    /// The computer's next bytes, or `nil` once it closed the tunnel.
    public func receive() async throws -> Data? {
        guard !finished else { return nil }
        while true {
            let frame = try await nextFrame()
            switch try transport.open(frame) {
            case let .data(bytes) where bytes.isEmpty: continue
            case let .data(bytes): return bytes
            case .closed:
                finished = true
                socket.cancel(with: .normalClosure, reason: nil)
                return nil
            }
        }
    }

    /// Closes the tunnel, telling the computer, then the WebSocket.
    public func close() async {
        guard !finished else { return }
        finished = true
        if let record = try? transport.sealClose(), let frame = try? Tunnel.Frame.encode(record) {
            try? await socket.send(.data(frame))
        }
        socket.cancel(with: .normalClosure, reason: nil)
    }
}
