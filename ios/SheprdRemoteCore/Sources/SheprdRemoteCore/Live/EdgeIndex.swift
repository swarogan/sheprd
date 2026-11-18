/// The index on the edge of a long list: each section's letter, then a dot
/// for each row it shows, so a list of a few sections still has a mark per
/// agent to aim at. Dots past what fits are thinned evenly, each section
/// keeping at least one; letters always stay.
public enum EdgeIndex {
    public enum Mark: Hashable, Sendable {
        case letter(String)
        /// One row, in its status's color.
        case dot(Home.AgentStatus)
    }

    public struct Entry: Hashable, Sendable {
        /// The id of the header or row a touch on it scrolls to.
        public var target: String
        public var mark: Mark

        public init(target: String, mark: Mark) {
            self.target = target
            self.mark = mark
        }
    }

    /// A section as the index reads it.
    public struct Part: Sendable {
        public var target: String
        public var title: String
        /// Its rows' ids and statuses, empty while folded.
        public var rows: [(id: String, status: Home.AgentStatus)]

        public init(target: String, title: String, rows: [(id: String, status: Home.AgentStatus)]) {
            self.target = target
            self.title = title
            self.rows = rows
        }
    }

    /// The entries for `parts`, at most `capacity` of them when the letters
    /// leave room for any dots at all.
    public static func entries(_ parts: [Part], capacity: Int) -> [Entry] {
        let rows = parts.reduce(0) { $0 + $1.rows.count }
        let room = max(capacity - parts.count, 0)
        return parts.flatMap { part -> [Entry] in
            let letter = Entry(target: part.target, mark: .letter(part.title.first.map { String($0).uppercased() } ?? "•"))
            return [letter] + kept(part.rows, rows: rows, room: room).map { Entry(target: $0.id, mark: .dot($0.status)) }
        }
    }

    /// `section`'s share of `room`, spread evenly across it from its first.
    private static func kept(
        _ section: [(id: String, status: Home.AgentStatus)],
        rows: Int,
        room: Int
    ) -> [(id: String, status: Home.AgentStatus)] {
        guard !section.isEmpty, rows > room else { return section }
        let share = min(section.count, max(1, room * section.count / max(rows, 1)))
        return (0..<share).map { section[$0 * section.count / share] }
    }
}
