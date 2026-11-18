import Foundation
import Testing
@testable import SheprdGitHub
@testable import SheprdRemoteCore

/// The vectors in `Fixtures/check_log_vectors.json` were cut by
/// `crates/sheprd-github/src/check_log.rs`; the phone must cut the same.
struct CheckLogTests {
    struct Vectors: Decodable {
        var maxExcerpt: Int
        var vectors: [Vector]

        enum CodingKeys: String, CodingKey {
            case vectors
            case maxExcerpt = "max_excerpt"
        }
    }

    struct Vector: Decodable {
        var name: String
        var log: String
        var excerpt: String
    }

    static func load() throws -> Vectors {
        let url = try #require(Bundle.module.url(forResource: "Fixtures/check_log_vectors", withExtension: "json"))
        return try JSONDecoder().decode(Vectors.self, from: Data(contentsOf: url))
    }

    @Test func theExcerptMatchesTheComputersForEveryVector() throws {
        let vectors = try Self.load()
        #expect(vectors.maxExcerpt == CheckLog.maxExcerpt)
        #expect(!vectors.vectors.isEmpty)
        for vector in vectors.vectors {
            #expect(CheckLog.excerpt(vector.log) == vector.excerpt, "\(vector.name)")
            #expect(vector.excerpt.utf8.count <= CheckLog.maxExcerpt, "\(vector.name)")
        }
    }

    @Test func aLogKeepsItsEndWhateverItsSize() {
        var tail = LogTail(keep: 7, limit: .max)
        var kept = true
        for _ in 0..<100 { kept = kept && tail.append(Array(repeating: UInt8(ascii: "a"), count: 1000)) }
        kept = kept && tail.append(Array("the end".utf8))
        #expect(kept)
        #expect(tail.bytes == Data("the end".utf8))
    }

    @Test func aLogThatEndsAtTheLimitIsReadToItsEnd() {
        var tail = LogTail(keep: 3, limit: 10)
        let kept = tail.append(Array("0123456789".utf8))
        #expect(kept)
        #expect(tail.bytes == Data("789".utf8))
    }

    @Test func aLogPastTheLimitIsRefusedRatherThanShownAsItsEnd() {
        var tail = LogTail(keep: 3, limit: 9)
        let kept = tail.append(Array("0123456789".utf8))
        #expect(!kept)
    }

    @Test func theTokenFreeRedirectMustBeEncrypted() {
        let url = "https://storage.example/log?sig=1"
        #expect(CheckLog.storage(status: 302, location: url)?.absoluteString == url)
        #expect(CheckLog.storage(status: 302, location: "http://storage.example/log") == nil)
        #expect(CheckLog.storage(status: 200, location: url) == nil)
        #expect(CheckLog.storage(status: 302, location: nil) == nil)
    }
}
