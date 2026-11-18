import Foundation
import Testing

/// What the relay tests share with the tunnel's: its vectors stay with
/// `SheprdTunnelTests`, where the Rust `sheprd-tunnel` tests write them, and
/// are read from there by path rather than copied.
enum Vectors {
    private struct Session: Decodable {
        let listener_identity: String
    }

    static func hex(_ text: String) throws -> Data {
        var bytes = Data()
        var index = text.startIndex
        while index < text.endIndex {
            let next = text.index(index, offsetBy: 2)
            bytes.append(try #require(UInt8(text[index..<next], radix: 16)))
            index = next
        }
        return bytes
    }

    /// The listener identity in the tunnel's session vector: a valid one,
    /// so a relay record made from it holds.
    static func listenerIdentity() throws -> Data {
        let url = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .appending(path: "SheprdTunnelTests/Fixtures/tunnel/session.json")
        let session = try JSONDecoder().decode(Session.self, from: Data(contentsOf: url))
        return try hex(session.listener_identity)
    }
}
