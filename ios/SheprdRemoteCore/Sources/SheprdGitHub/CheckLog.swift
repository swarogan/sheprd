import Foundation

/// The excerpt of a GitHub Actions job's log that a prompt carries, cut on
/// the phone exactly as `crates/sheprd-github/src/check_log.rs` cuts it on
/// the computer: the lines leading to the last `##[error]`, or the log's
/// end when it marks none, without timestamps, colors, or control and
/// direction characters, within 8 KiB and starting on a whole line.
/// `Fixtures/check_log_vectors.json` holds the vectors both must agree on.
/// Log text is untrusted.
enum CheckLog {
    /// Bytes of an excerpt at most.
    static let maxExcerpt = 8 * 1024
    /// Lines of an excerpt before the failure it ends on.
    static let linesBefore = 80
    /// Bytes kept of the end of a log.
    static let tail = 256 * 1024
    /// Bytes of a log read at most: past them its end, and so its failure,
    /// is out of reach.
    static let readLimit = 64 * 1024 * 1024
    /// What GitHub Actions writes before a line that failed the step.
    static let errorMark = "##[error]"

    /// What a prompt quotes of `log`.
    static func excerpt(_ log: String) -> String {
        let lines = Self.lines(log).map(plain)
        let end = lines.lastIndex { $0.contains(errorMark) }.map { $0 + 1 } ?? lines.count
        let start = max(end - (linesBefore + 1), 0)
        let text = Array(lines[start..<end].joined(separator: "\n").utf8)
        var from = max(text.count - maxExcerpt, 0)
        // Never start inside a character.
        while from < text.count, text[from] & 0xC0 == 0x80 { from += 1 }
        var kept = text[from...]
        // A cut line reads as a mistake; the next whole one starts better.
        if from > 0, let newline = kept.firstIndex(of: UInt8(ascii: "\n")) {
            kept = kept[(newline + 1)...]
        }
        return trimmed(String(decoding: kept, as: UTF8.self))
    }

    /// Lines as Rust's `str::lines` splits them: at `\n`, a `\r` before it
    /// dropped, and no empty line after a final newline. Split by scalar,
    /// as Swift keeps `\r\n` as one `Character`.
    static func lines(_ log: String) -> [String] {
        var pieces = log.unicodeScalars.split(separator: "\n", omittingEmptySubsequences: false)
        if pieces.last?.isEmpty == true { pieces.removeLast() }
        return pieces.map { piece in
            var line = String.UnicodeScalarView(piece)
            if line.last == "\r" { line.removeLast() }
            return String(line)
        }
    }

    /// A log line without its timestamp, colors, or anything else that is
    /// not text.
    static func plain(_ line: String) -> String {
        var scalars = Substring(line).unicodeScalars[...]
        if scalars.first == "\u{FEFF}" { scalars.removeFirst() }
        if let space = scalars.firstIndex(of: " "), timestamp(Substring(scalars[..<space])) {
            scalars = scalars[scalars.index(after: space)...]
        }
        var text = String.UnicodeScalarView()
        var iterator = scalars.makeIterator()
        while let scalar = iterator.next() {
            if scalar == "\u{1B}" {
                // A color: `ESC [`, parameters, then one final letter.
                if iterator.next() == "[" {
                    while let next = iterator.next(), !(0x40...0x7E).contains(next.value) {}
                }
            } else if scalar == "\t" {
                text.append(contentsOf: "    ".unicodeScalars)
            } else if scalar.properties.generalCategory != .control, !hidden(scalar) {
                text.append(scalar)
            }
        }
        return String(text)
    }

    /// Direction overrides and isolates, which would reorder what a reader
    /// sees.
    private static func hidden(_ scalar: Unicode.Scalar) -> Bool {
        (0x202A...0x202E).contains(scalar.value) || (0x2066...0x2069).contains(scalar.value)
    }

    /// Whether `word` is the time GitHub Actions starts each line with.
    static func timestamp(_ word: Substring) -> Bool {
        let bytes = Array(word.utf8)
        return bytes.count >= 20 && bytes.last == UInt8(ascii: "Z") && bytes[10] == UInt8(ascii: "T")
    }

    /// `text` without leading and trailing white space, as Rust's `trim`
    /// sees it: the Unicode `White_Space` property.
    private static func trimmed(_ text: String) -> String {
        let scalars = text.unicodeScalars
        guard let first = scalars.firstIndex(where: { !$0.properties.isWhitespace }),
              let last = scalars.lastIndex(where: { !$0.properties.isWhitespace })
        else { return "" }
        return String(scalars[first...last])
    }

    /// Where a log request's redirect leads. It carries no token, but a
    /// plain address would still let the network rewrite the log.
    static func storage(status: Int, location: String?) -> URL? {
        guard (300..<400).contains(status), let location, location.hasPrefix("https://") else { return nil }
        return URL(string: location)
    }
}

/// The last `keep` bytes of a stream that must end within `limit` bytes:
/// what is read by then is not the end, and would pass for it.
struct LogTail: Sendable {
    let keep: Int
    let limit: Int
    private(set) var total = 0
    private var buffer = Data()

    init(keep: Int, limit: Int) {
        self.keep = keep
        self.limit = limit
    }

    /// Adds what was read next; false once the stream is past `limit`.
    mutating func append(_ bytes: some Collection<UInt8>) -> Bool {
        total += bytes.count
        guard total <= limit else { return false }
        buffer.append(contentsOf: bytes)
        // Dropped in steps, so a long log is not copied once per chunk.
        if buffer.count > keep * 2 { buffer.removeFirst(buffer.count - keep) }
        return true
    }

    var bytes: Data { buffer.suffix(keep) }
}
