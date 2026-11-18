import Foundation

/// The HTML a GitHub comment's Markdown may hold, as Markdown, so the app's
/// native Markdown view draws it rather than showing its tags. Bots such as
/// review tools write headings, badges, links, details, and tables in HTML.
///
/// Only the tags GitHub keeps in a comment are read, and only outside code:
/// `Vec<T>` in prose or anything in a code block stays as written. Images
/// become their alt text, as the view draws no remote pictures; links keep
/// only `http` and `https` addresses.
public enum CommentHTML {
    public static func markdown(_ text: String) -> String {
        guard text.contains("<") || text.contains("&") else { return text }
        // Fenced code is kept as is; the prose between converts.
        var result = ""
        var prose = ""
        var fence = false
        for line in text.split(separator: "\n", omittingEmptySubsequences: false) {
            if line.trimmingCharacters(in: .whitespaces).hasPrefix("```") {
                if !fence {
                    result += Converter.convert(prose)
                    prose = ""
                }
                fence.toggle()
                result += line + "\n"
            } else if fence {
                result += line + "\n"
            } else {
                prose += line + "\n"
            }
        }
        result += Converter.convert(prose)
        return tidy(result)
    }

    /// At most one blank line in a row, outside fences, and no blank edges.
    private static func tidy(_ text: String) -> String {
        var lines: [Substring] = []
        var fence = false
        for line in text.split(separator: "\n", omittingEmptySubsequences: false) {
            let trimmed = line.trimmingCharacters(in: .whitespaces)
            if trimmed.hasPrefix("```") { fence.toggle() }
            if !fence, trimmed.isEmpty, lines.last?.trimmingCharacters(in: .whitespaces).isEmpty ?? true { continue }
            // Trailing spaces stay: two of them are a `<br>`'s line break.
            lines.append(line)
        }
        return lines.joined(separator: "\n").trimmingCharacters(in: .whitespacesAndNewlines)
    }

    /// The tags read; any other `<…>` is text.
    static let tags: Set<String> = [
        // keep-sorted start
        "a", "b", "blockquote", "br", "code", "del", "details", "div", "em", "h1", "h2", "h3", "h4", "h5", "h6",
        "hr", "i", "img", "kbd", "li", "ol", "p", "picture", "pre", "s", "source", "span", "strong", "sub",
        "summary", "sup", "table", "tbody", "td", "th", "thead", "tr", "ul",
        // keep-sorted end
    ]

    static let entities: [String: String] = [
        "amp": "&", "apos": "'", "gt": ">", "lt": "<", "mdash": "—", "nbsp": " ", "ndash": "–", "quot": "\"",
    ]

    /// `&amp;`, `&#39;`, `&#x2014;` and the like as their characters.
    static func decode(_ text: some StringProtocol) -> String {
        guard text.contains("&") else { return String(text) }
        return String(text).replacing(/&(#x[0-9a-fA-F]{1,6}|#[0-9]{1,7}|[a-zA-Z]{2,8});/) { match in
            let name = String(match.output.1)
            if name.hasPrefix("#x"), let value = UInt32(name.dropFirst(2), radix: 16), let scalar = Unicode.Scalar(value) {
                return String(Character(scalar))
            }
            if name.hasPrefix("#"), let value = UInt32(name.dropFirst()), let scalar = Unicode.Scalar(value) {
                return String(Character(scalar))
            }
            return entities[name.lowercased()] ?? String(match.output.0)
        }
    }

    /// One run of prose: tags out, Markdown in.
    private struct Converter {
        let text: String
        var out = ""
        /// Open links' addresses, innermost last; nil for one not kept.
        var links: [String?] = []
        var lists: [Int?] = []
        var row: [String]?
        var cell: String?
        var rows: [[String]] = []
        var inPre = false

        init(_ text: String) { self.text = text }

        static func convert(_ text: String) -> String {
            var converter = Converter(text)
            return converter.run()
        }

        mutating func run() -> String {
            var rest = Substring(text)
            while let open = rest.firstIndex(where: { $0 == "<" || $0 == "`" }) {
                emit(rest[..<open])
                let tail = rest[open...]
                if tail.hasPrefix("`") {
                    // An inline code span, as written: its tags are text.
                    let ticks = tail.prefix { $0 == "`" }
                    let body = tail[ticks.endIndex...]
                    guard let close = body.range(of: ticks) else {
                        write(String(ticks))
                        rest = body
                        continue
                    }
                    write(String(tail[..<close.upperBound]))
                    rest = tail[close.upperBound...]
                    continue
                }
                if tail.hasPrefix("<!--") {
                    rest = tail.range(of: "-->").map { tail[$0.upperBound...] } ?? ""
                    continue
                }
                if let tag = Tag(tail), CommentHTML.tags.contains(tag.name) {
                    handle(tag)
                    rest = tail[tag.end...]
                } else {
                    emit(tail.prefix(1))
                    rest = tail.dropFirst()
                }
            }
            emit(rest)
            return out
        }

        /// Text between tags and code spans, its entities decoded.
        mutating func emit(_ text: Substring) {
            guard !text.isEmpty else { return }
            write(CommentHTML.decode(text))
        }

        mutating func write(_ text: String) {
            if cell != nil {
                cell! += text.replacingOccurrences(of: "\n", with: " ").replacingOccurrences(of: "|", with: "\\|")
            } else {
                out += text
            }
        }

        mutating func handle(_ tag: Tag) {
            switch tag.name {
            case "a":
                if tag.closing {
                    guard let link = links.popLast() else { return }
                    if let link { write("](\(link))") }
                } else {
                    let href = tag.attribute("href").flatMap(URL.init(string:))
                    let kept = href.flatMap { ["http", "https"].contains($0.scheme?.lowercased() ?? "") ? $0.absoluteString : nil }
                    links.append(kept)
                    if kept != nil { write("[") }
                }
            case "img":
                guard !tag.closing, let alt = tag.attribute("alt")?.trimmingCharacters(in: .whitespaces), !alt.isEmpty else { return }
                // A badge's few letters read as a chip; longer alt text as text.
                write(alt.count <= 12 && !alt.contains(" ") && links.isEmpty ? "`\(alt)`" : alt)
            case "br":
                write(cell == nil ? "  \n" : " ")
            case "hr":
                block("---")
            case "p", "div", "blockquote", "details":
                block("")
            case "summary":
                if tag.closing { write("**\n\n") } else {
                    block("")
                    write("**")
                }
            case "h1", "h2", "h3", "h4", "h5", "h6":
                if tag.closing { write("\n\n") } else {
                    block("")
                    write(String(repeating: "#", count: Int(tag.name.dropFirst()) ?? 2) + " ")
                }
            case "b", "strong":
                write("**")
            case "i", "em":
                write("_")
            case "del", "s":
                write("~~")
            case "code", "kbd":
                if !inPre { write("`") }
            case "pre":
                inPre = !tag.closing
                write(tag.closing ? "\n```\n\n" : "\n\n```\n")
            case "ul":
                tag.closing ? (_ = lists.popLast()) : lists.append(nil)
                if tag.closing { write("\n") }
            case "ol":
                tag.closing ? (_ = lists.popLast()) : lists.append(0)
                if tag.closing { write("\n") }
            case "li":
                guard !tag.closing else { return }
                let indent = String(repeating: "  ", count: max(lists.count - 1, 0))
                if let number = lists.last ?? nil {
                    lists[lists.count - 1] = number + 1
                    write("\n\(indent)\(number + 1). ")
                } else {
                    write("\n\(indent)- ")
                }
            case "table":
                if tag.closing { flushTable() } else { rows = [] }
            case "tr":
                if tag.closing {
                    if let row { rows.append(row) }
                    row = nil
                } else {
                    row = []
                }
            case "td", "th":
                if tag.closing {
                    if let text = cell { row?.append(text.trimmingCharacters(in: .whitespaces)) }
                    cell = nil
                } else {
                    cell = ""
                }
            default:
                // picture, source, span, sub, sup, thead, tbody: their
                // content only.
                break
            }
        }

        /// Starts a block on a line of its own.
        mutating func block(_ text: String) {
            write("\n\n" + text + (text.isEmpty ? "" : "\n\n"))
        }

        mutating func flushTable() {
            defer { rows = [] }
            guard let header = rows.first, !header.isEmpty else { return }
            let width = rows.map(\.count).max() ?? header.count
            let line = { (cells: [String]) in
                "| " + (cells + Array(repeating: "", count: width - cells.count)).joined(separator: " | ") + " |"
            }
            var table = [line(header), "|" + Array(repeating: " --- |", count: width).joined()]
            table += rows.dropFirst().map(line)
            block(table.joined(separator: "\n"))
        }
    }

    /// A tag read from `<` to `>`: its name, whether it closes, and its
    /// attributes' text.
    struct Tag {
        var name: String
        var closing: Bool
        var attributes: Substring
        var end: Substring.Index

        /// The longest tag read; a longer `<…` is text, so a body full of
        /// unclosed `<` stays linear to read.
        static let maxLength = 1024

        /// Read in one pass, no regular expression: the name, then to the
        /// `>` outside quotes, within `maxLength` characters.
        init?(_ text: Substring) {
            let text = text.prefix(Self.maxLength)
            var index = text.index(after: text.startIndex)
            closing = index < text.endIndex && text[index] == "/"
            if closing { index = text.index(after: index) }
            let nameStart = index
            guard index < text.endIndex, text[index].isASCII, text[index].isLetter else { return nil }
            while index < text.endIndex, text[index].isASCII, text[index].isLetter || text[index].isNumber {
                index = text.index(after: index)
            }
            name = text[nameStart..<index].lowercased()
            let attributesStart = index
            var quote: Character?
            while index < text.endIndex {
                let character = text[index]
                if let open = quote {
                    if character == open { quote = nil }
                } else if character == "\"" || character == "'" {
                    quote = character
                } else if character == ">" {
                    attributes = text[attributesStart..<index]
                    end = text.index(after: index)
                    return
                }
                index = text.index(after: index)
            }
            return nil
        }

        func attribute(_ wanted: String) -> String? {
            for match in attributes.matches(of: /([a-zA-Z-:]+)\s*=\s*("([^"]*)"|'([^']*)'|([^\s"'>]+))/) {
                guard match.output.1.lowercased() == wanted else { continue }
                let value = match.output.3 ?? match.output.4 ?? match.output.5 ?? ""
                return CommentHTML.decode(value)
            }
            return nil
        }
    }
}
