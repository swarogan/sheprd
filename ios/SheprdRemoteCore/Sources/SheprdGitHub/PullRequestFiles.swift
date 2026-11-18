import Foundation
import SheprdRemoteCore

/// The files a pull request changes, from GitHub's REST
/// `GET /repos/{owner}/{repo}/pulls/{number}/files`: each with its counts
/// and its patch as the Changes screen's diff lines. Bounded: at most
/// `pageLimit` pages of `perPage`, each page at most `pageBytes`, each
/// patch at most `patchBytes`, and `patchBudget` for all of them; past a
/// bound a file still lists, its lines replaced by a note.
public struct PullRequestFiles: Hashable, Sendable {
    static let perPage = 100
    static let pageLimit = 3
    static let pageBytes = 4 * 1024 * 1024
    static let patchBytes = 128 * 1024
    static let patchBudget = 2 * 1024 * 1024

    public var files: [PullRequestFile]
    /// GitHub has more files than were read.
    public var truncated: Bool

    public init(files: [PullRequestFile], truncated: Bool) {
        self.files = files
        self.truncated = truncated
    }

    public var additions: Int { files.reduce(0) { $0 + ($1.file.added ?? 0) } }
    public var deletions: Int { files.reduce(0) { $0 + ($1.file.removed ?? 0) } }

    /// The path of one page.
    static func path(owner: String, repo: String, number: UInt64, page: Int) -> String {
        "/repos/\(GitHubAPI.segment(owner))/\(GitHubAPI.segment(repo))/pulls/\(number)/files?per_page=\(perPage)&page=\(page)"
    }

    /// One page's files, each patch counted against `budget`, which is
    /// what remains of `patchBudget`.
    static func parse(page: Data, budget: inout Int) throws(GitHubError) -> [PullRequestFile] {
        guard let entries = try? JSONDecoder().decode([Entry?].self, from: page) else { throw .malformed }
        return entries.compactMap { $0?.file(budget: &budget) }
    }

    struct Entry: Decodable {
        var filename: String
        var previous_filename: String?
        var status: String?
        var additions: Int?
        var deletions: Int?
        var changes: Int?
        var patch: String?

        func file(budget: inout Int) -> PullRequestFile? {
            let path = PullRequestAnswer.clean(filename)
            guard !path.isEmpty, path.utf8.count <= 4096 else { return nil }
            let added = max(additions ?? 0, 0)
            let removed = max(deletions ?? 0, 0)
            let status: ChangedFile.Status = switch status {
            case "added", "copied": .added
            case "removed": .deleted
            case "renamed": .renamed
            default: .modified
            }
            let shown: DiffShown
            var lines: [DiffLine] = []
            if let patch {
                let size = patch.utf8.count
                if size > PullRequestFiles.patchBytes || size > budget {
                    shown = .tooLarge
                } else {
                    budget -= size
                    shown = .text
                    lines = PullRequestFiles.lines(patch)
                }
            } else if (changes ?? added + removed) > 0 {
                // GitHub leaves out the patch of a change too large to show.
                shown = .tooLarge
            } else if status == .renamed {
                shown = .text
            } else {
                shown = .binary
            }
            let file = ChangedFile(
                path: path,
                oldPath: previous_filename.map(PullRequestAnswer.clean),
                status: status,
                added: added,
                removed: removed,
                shown: shown
            )
            return PullRequestFile(file: file, diff: FileDiff(path: path, shown: shown, lines: lines))
        }
    }

    /// A unified diff's lines, numbered from its hunk headers.
    static func lines(_ patch: String) -> [DiffLine] {
        var old: UInt32 = 0
        var new: UInt32 = 0
        var lines: [DiffLine] = []
        for raw in patch.split(separator: "\n", omittingEmptySubsequences: false) {
            let text = shown(raw)
            if text.hasPrefix("@@") {
                (old, new) = hunk(text) ?? (old, new)
                lines.append(DiffLine(kind: .hunk, text: text))
                continue
            }
            switch text.first {
            case "+":
                lines.append(DiffLine(kind: .added, new: new, text: String(text.dropFirst())))
                new &+= 1
            case "-":
                lines.append(DiffLine(kind: .removed, old: old, text: String(text.dropFirst())))
                old &+= 1
            case " ":
                lines.append(DiffLine(kind: .context, old: old, new: new, text: String(text.dropFirst())))
                old &+= 1
                new &+= 1
            case nil where raw.isEmpty && lines.isEmpty:
                continue
            default:
                lines.append(DiffLine(kind: .meta, text: text))
            }
        }
        // A patch's last line ends in a newline that starts no line.
        if lines.last == DiffLine(kind: .meta, text: "") { lines.removeLast() }
        return lines
    }

    /// The first line numbers of `@@ -OLD[,N] +NEW[,N] @@`.
    static func hunk(_ header: String) -> (UInt32, UInt32)? {
        let fields = header.split(separator: " ")
        guard fields.count >= 3, fields[1].hasPrefix("-"), fields[2].hasPrefix("+"),
              let old = UInt32(fields[1].dropFirst().prefix { $0 != "," }),
              let new = UInt32(fields[2].dropFirst().prefix { $0 != "," })
        else { return nil }
        return (old, new)
    }

    /// A line of remote text as a diff row shows it: tabs kept, other
    /// control and direction characters dropped.
    static func shown(_ line: Substring) -> String {
        String(String.UnicodeScalarView(line.unicodeScalars.filter { scalar in
            scalar == "\t" || (scalar.properties.generalCategory != .control
                && !(0x202A...0x202E).contains(scalar.value) && !(0x2066...0x2069).contains(scalar.value))
        }))
    }
}

/// One changed file: its row, and its patch as the diff screen shows it.
public struct PullRequestFile: Hashable, Sendable, Identifiable {
    public var file: ChangedFile
    public var diff: FileDiff

    public var id: String { file.path }
}
