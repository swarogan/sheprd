import CryptoKit
import Foundation
import SheprdRemoteCore
import SheprdTunnel

/// What a dialer proves to a relay over its WebSocket fallback, as
/// `sheprd-relay`'s `socket` module checks it.
public enum RelayProof {
    static let dialContext = Data("sheprd-relay/dial/v1\0".utf8)
    public static let nonceLength = 32

    /// The bytes a dialer signs to reach `route` on the relay `name`.
    public static func dialSigned(name: String, nonce: Data, route: Data) -> Data {
        var nameBytes = Data(name.utf8)
        if nameBytes.count > 255 { nameBytes = nameBytes.prefix(255) }
        return dialContext + nameBytes + Data([0]) + nonce + route
    }

    static let pairingDialContext = Data("sheprd-relay/pairing-dial/v1\0".utf8)

    /// The one-dial key a phone pairs through a relay with, from the
    /// pairing secret: as `sheprd-relay`'s `pairing_dial_key`, the first
    /// SHA-256 of context, secret, and a counter that is a P-256 scalar.
    public static func pairingDialKey(secret: Data) -> P256.Signing.PrivateKey {
        var counter: UInt8 = 0
        while true {
            let digest = Data(SHA256.hash(data: pairingDialContext + secret + Data([counter])))
            if let key = try? P256.Signing.PrivateKey(rawRepresentation: digest) { return key }
            counter &+= 1
        }
    }

    /// The dialer's hello: its key, SEC1 compressed, and its signature,
    /// `r ‖ s`, both base64url.
    public static func hello(key: P256.Signing.PrivateKey, signed: Data) throws -> [String: String] {
        let signature = try key.signature(for: signed)
        return [
            "key": Base64URL.encode(key.publicKey.compressedRepresentation),
            "signature": Base64URL.encode(signature.rawRepresentation),
        ]
    }
}
