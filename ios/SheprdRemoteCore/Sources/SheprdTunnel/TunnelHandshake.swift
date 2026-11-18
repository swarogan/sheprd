import CryptoKit
import Foundation

/// The end-to-end tunnel inside a relay stream, the dialer's side: the same
/// protocol as the daemon's `sheprd-tunnel` crate, which writes the vectors
/// `TunnelTests` checks this against. A relay sees every byte it joins;
/// this is what keeps it from reading, changing, or posing as either side
/// (`docs/relay-plan.md`, End-to-end encryption).
public enum Tunnel {
    public static let version: UInt8 = 1
    static let prologueTag = Data("sheprd-tunnel/v1\0".utf8)
    static let fingerprintContext = Data("sheprd-tunnel/identity/v1\0".utf8)
    static let pairingSalt = Data("sheprd-tunnel/pairing/v1".utf8)
    static let codeSalt = Data("sheprd-tunnel/pairing-code/v1".utf8)
    public static let x25519Length = 32
    public static let mlkemPublicLength = 1_184
    public static let mlkemCiphertextLength = 1_088
    public static let identityLength = x25519Length + mlkemPublicLength

    public enum Failure: Error, Equatable {
        case identity
        case malformed
        case handshake
        case record
        case frameTooLong
    }

    enum Mode: UInt8 {
        case session = 1
        case pairing = 2
    }

    /// A listener's public keys, pinned at pairing: X25519 for Noise, and
    /// ML-KEM-768 for the hybrid.
    public struct Identity: Equatable, Sendable {
        public let x25519: Data
        public let mlkem: Data

        public init(bytes: Data) throws {
            guard bytes.count == Tunnel.identityLength else { throw Failure.identity }
            let x25519 = Data(bytes.prefix(Tunnel.x25519Length))
            let mlkem = Data(bytes.suffix(Tunnel.mlkemPublicLength))
            guard (try? MLKEM768.PublicKey(rawRepresentation: mlkem)) != nil,
                  (try? Curve25519.KeyAgreement.PublicKey(rawRepresentation: x25519)) != nil
            else { throw Failure.identity }
            self.x25519 = x25519
            self.mlkem = mlkem
        }

        public var bytes: Data { x25519 + mlkem }

        /// The SHA-256 the QR carries and every prologue binds.
        public var fingerprint: Data {
            Data(SHA256.hash(data: Tunnel.fingerprintContext + x25519 + mlkem))
        }
    }

    static func prologue(_ mode: Mode, route: Data, listener: Data) -> Data {
        prologueTag + Data([version, mode.rawValue]) + route + listener
    }

    /// The pairing handshake's PSK, from the QR's secret.
    public static func pairingPSK(secret: Data) -> Data {
        HKDF<SHA256>.deriveKey(
            inputKeyMaterial: SymmetricKey(data: secret),
            salt: pairingSalt,
            info: Data("psk".utf8),
            outputByteCount: 32
        ).data
    }

    /// The six-digit code both screens show when pairing.
    public static func pairingCode(handshakeHash: Data) -> UInt32 {
        let bytes = HKDF<SHA256>.deriveKey(
            inputKeyMaterial: SymmetricKey(data: handshakeHash),
            salt: codeSalt,
            info: Data("code".utf8),
            outputByteCount: 4
        ).data
        let value = bytes.reduce(UInt32(0)) { ($0 << 8) | UInt32($1) }
        return value % 1_000_000
    }

    /// An ML-KEM-768 ciphertext and the shared secret in it.
    public typealias Encapsulation = (ciphertext: Data, shared: Data)

    /// The dialer's side of the handshake, between its first message and
    /// the listener's answer.
    public struct Initiator {
        var state: Noise.SymmetricState
        let ephemeral: Curve25519.KeyAgreement.PrivateKey
        let local: Curve25519.KeyAgreement.PrivateKey
        let psk: Data

        /// Dials a paired listener whose `identity` was pinned at pairing,
        /// on `route`. Returns the first message to send.
        public static func session(
            local: Curve25519.KeyAgreement.PrivateKey,
            listener identity: Identity,
            route: Data,
            ephemeral: Curve25519.KeyAgreement.PrivateKey = .init(),
            encapsulate: ((MLKEM768.PublicKey) throws -> Encapsulation)? = nil
        ) throws -> (Initiator, Data) {
            let key = try MLKEM768.PublicKey(rawRepresentation: identity.mlkem)
            let encapsulated: Encapsulation
            if let encapsulate {
                encapsulated = try encapsulate(key)
            } else {
                let result = try key.encapsulate()
                encapsulated = (result.encapsulated, result.sharedSecret.data)
            }
            let prologue = Tunnel.prologue(.session, route: route, listener: identity.fingerprint)
            return try start(
                local: local, listener: identity.x25519, prologue: prologue, ephemeral: ephemeral,
                psk: encapsulated.shared, payload: Data([Tunnel.version]) + encapsulated.ciphertext
            )
        }

        /// Pairs with a listener known only by the QR: its X25519 key, its
        /// identity's fingerprint, its route, and the pairing secret.
        public static func pairing(
            local: Curve25519.KeyAgreement.PrivateKey,
            listenerX25519: Data,
            listenerFingerprint: Data,
            route: Data,
            secret: Data,
            ephemeral: Curve25519.KeyAgreement.PrivateKey = .init()
        ) throws -> (Initiator, Data) {
            let prologue = Tunnel.prologue(.pairing, route: route, listener: listenerFingerprint)
            return try start(
                local: local, listener: listenerX25519, prologue: prologue, ephemeral: ephemeral,
                psk: Tunnel.pairingPSK(secret: secret), payload: Data([Tunnel.version])
            )
        }

        /// `-> e, es, s, ss` with `payload`, after the pre-message `<- s`.
        static func start(
            local: Curve25519.KeyAgreement.PrivateKey,
            listener: Data,
            prologue: Data,
            ephemeral: Curve25519.KeyAgreement.PrivateKey,
            psk: Data,
            payload: Data
        ) throws -> (Initiator, Data) {
            let remote = try Curve25519.KeyAgreement.PublicKey(rawRepresentation: listener)
            var state = Noise.SymmetricState()
            state.mixHash(prologue)
            state.mixHash(listener)
            let ephemeralPublic = ephemeral.publicKey.rawRepresentation
            var message = ephemeralPublic
            state.mixHash(ephemeralPublic)
            // A PSK pattern also mixes `e` into the key.
            state.mixKey(ephemeralPublic)
            state.mixKey(try ephemeral.sharedSecretFromKeyAgreement(with: remote).data)
            message += try state.encryptAndHash(local.publicKey.rawRepresentation)
            state.mixKey(try local.sharedSecretFromKeyAgreement(with: remote).data)
            message += try state.encryptAndHash(payload)
            return (Initiator(state: state, ephemeral: ephemeral, local: local, psk: psk), message)
        }

        /// Reads the listener's answer, `<- e, ee, se, psk`: the tunnel,
        /// and the handshake's hash (for `pairingCode`).
        public func finish(answer: Data) throws -> (Transport, Data) {
            guard answer.count >= Tunnel.x25519Length + Noise.tagLength else { throw Failure.malformed }
            var state = self.state
            let remoteEphemeralBytes = Data(answer.prefix(Tunnel.x25519Length))
            let remoteEphemeral = try Curve25519.KeyAgreement.PublicKey(rawRepresentation: remoteEphemeralBytes)
            state.mixHash(remoteEphemeralBytes)
            state.mixKey(remoteEphemeralBytes)
            state.mixKey(try ephemeral.sharedSecretFromKeyAgreement(with: remoteEphemeral).data)
            state.mixKey(try local.sharedSecretFromKeyAgreement(with: remoteEphemeral).data)
            state.mixKeyAndHash(psk)
            let payload: Data
            do {
                payload = try state.decryptAndHash(Data(answer.dropFirst(Tunnel.x25519Length)))
            } catch {
                throw Failure.handshake
            }
            guard payload == Data([Tunnel.version]) else { throw Failure.malformed }
            let (send, receive) = state.split()
            return (Transport(send: send, receive: receive), state.hash)
        }
    }
}
