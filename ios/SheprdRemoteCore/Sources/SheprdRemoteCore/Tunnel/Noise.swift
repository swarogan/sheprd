import CryptoKit
import Foundation

/// The parts of the Noise Protocol Framework (revision 34) the tunnel uses,
/// for `Noise_IKpsk2_25519_ChaChaPoly_SHA256`: a cipher state and the
/// symmetric state that runs the handshake. The daemon's side is the
/// `snow` crate; `TunnelTests` checks this against its vectors byte for
/// byte, so each step here follows the specification's names.
enum Noise {
    static let protocolName = "Noise_IKpsk2_25519_ChaChaPoly_SHA256"
    static let tagLength = 16

    enum Failure: Error, Equatable {
        case decrypt
        case nonceExhausted
    }

    /// `k` and `n`: ChaCha20-Poly1305 with a 64-bit counter.
    struct CipherState {
        var key: SymmetricKey?
        var nonce: UInt64 = 0

        init(key: SymmetricKey? = nil) {
            self.key = key
        }

        /// Noise's ChaChaPoly nonce: four zero bytes, then the counter,
        /// little-endian.
        static func nonce(_ n: UInt64) throws -> ChaChaPoly.Nonce {
            var bytes = Data(count: 4)
            withUnsafeBytes(of: n.littleEndian) { bytes.append(contentsOf: $0) }
            return try ChaChaPoly.Nonce(data: bytes)
        }

        mutating func encrypt(_ plaintext: Data, ad: Data) throws -> Data {
            guard let key else { return plaintext }
            guard nonce < UInt64.max else { throw Failure.nonceExhausted }
            let box = try ChaChaPoly.seal(plaintext, using: key, nonce: Self.nonce(nonce), authenticating: ad)
            nonce += 1
            return box.ciphertext + box.tag
        }

        mutating func decrypt(_ ciphertext: Data, ad: Data) throws -> Data {
            guard let key else { return ciphertext }
            guard nonce < UInt64.max else { throw Failure.nonceExhausted }
            guard ciphertext.count >= Noise.tagLength else { throw Failure.decrypt }
            let split = ciphertext.count - Noise.tagLength
            do {
                let box = try ChaChaPoly.SealedBox(
                    nonce: Self.nonce(nonce),
                    ciphertext: ciphertext.prefix(split),
                    tag: ciphertext.suffix(Noise.tagLength)
                )
                let plaintext = try ChaChaPoly.open(box, using: key, authenticating: ad)
                nonce += 1
                return plaintext
            } catch {
                throw Failure.decrypt
            }
        }

        /// `REKEY(k)`: the first 32 bytes of encrypting 32 zeros at the
        /// largest nonce. The counter carries on.
        mutating func rekey() throws {
            guard let key else { return }
            let box = try ChaChaPoly.seal(Data(count: 32), using: key, nonce: Self.nonce(UInt64.max), authenticating: Data())
            self.key = SymmetricKey(data: box.ciphertext.prefix(32))
        }
    }

    /// `ck`, `h`, and the cipher state, through the handshake.
    struct SymmetricState {
        var chainingKey: Data
        var hash: Data
        var cipher = CipherState()

        init(protocolName: String = Noise.protocolName) {
            let name = Data(protocolName.utf8)
            hash = name.count <= 32 ? name + Data(count: 32 - name.count) : Data(SHA256.hash(data: name))
            chainingKey = hash
        }

        mutating func mixHash(_ data: Data) {
            hash = Data(SHA256.hash(data: hash + data))
        }

        mutating func mixKey(_ input: Data) {
            let (ck, key) = Self.hkdf2(chainingKey, input)
            chainingKey = ck
            cipher = CipherState(key: SymmetricKey(data: key))
        }

        mutating func mixKeyAndHash(_ input: Data) {
            let outputs = Self.hkdf(chainingKey, input, count: 3)
            chainingKey = outputs[0]
            mixHash(outputs[1])
            cipher = CipherState(key: SymmetricKey(data: outputs[2]))
        }

        mutating func encryptAndHash(_ plaintext: Data) throws -> Data {
            let ciphertext = try cipher.encrypt(plaintext, ad: hash)
            mixHash(ciphertext)
            return ciphertext
        }

        mutating func decryptAndHash(_ ciphertext: Data) throws -> Data {
            let plaintext = try cipher.decrypt(ciphertext, ad: hash)
            mixHash(ciphertext)
            return plaintext
        }

        /// The two transport cipher states: the initiator sends with the
        /// first.
        func split() -> (CipherState, CipherState) {
            let (first, second) = Self.hkdf2(chainingKey, Data())
            return (CipherState(key: SymmetricKey(data: first)), CipherState(key: SymmetricKey(data: second)))
        }

        static func hkdf2(_ chainingKey: Data, _ input: Data) -> (Data, Data) {
            let outputs = hkdf(chainingKey, input, count: 2)
            return (outputs[0], outputs[1])
        }

        /// Noise's `HKDF`: HMAC-SHA256 with the chaining key as salt.
        static func hkdf(_ chainingKey: Data, _ input: Data, count: Int) -> [Data] {
            let temp = Data(HMAC<SHA256>.authenticationCode(for: input, using: SymmetricKey(data: chainingKey)))
            var outputs: [Data] = []
            var previous = Data()
            for index in 1...count {
                let block = previous + Data([UInt8(index)])
                previous = Data(HMAC<SHA256>.authenticationCode(for: block, using: SymmetricKey(data: temp)))
                outputs.append(previous)
            }
            return outputs
        }
    }
}

extension SharedSecret {
    var data: Data { withUnsafeBytes { Data($0) } }
}

extension SymmetricKey {
    var data: Data { withUnsafeBytes { Data($0) } }
}
