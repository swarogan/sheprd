import Foundation

/// Asking a workspace's agent chat for a short name: the prompt the phone
/// sends, and the name read back from the chat's transcript once the turn
/// that answers it ends. The phone then renames the workspace itself, as
/// an agent has no way to rename one Sheprd keeps for Herdr 2.
public enum WorkspaceNaming {
    /// The most characters a suggested name keeps; a title bar on a phone
    /// shows about this many beside its Back button.
    public static let maxLength = 28

    /// What the agent is asked. The limits are spelled out because agents
    /// otherwise answer with a sentence, a list of choices, or a branch name.
    public static func prompt(current: String) -> String {
        """
        Suggest a short name for this workspace, based on the work in this conversation. \
        It is currently named "\(current)". \
        Reply with the name only: 2 to 4 plain words, at most \(maxLength) characters, \
        no quotes, punctuation, emoji, or explanation. \
        Do not run tools or change any files.
        """
    }

    /// The name the agent gave in answer to `prompt`, once its turn has
    /// ended; `nil` while it is still answering, or when the prompt is not
    /// in the transcript (yet). Only a prompt entered after `after`, the
    /// transcript's last entry when it was sent, counts, so a retry with
    /// the same words waits for its own answer behind a busy agent. A turn
    /// that was stopped, refused, or failed, or that ended without a usable
    /// name, answers ``Answer/noName``, whatever text it had streamed.
    public static func answer(to prompt: String, in transcript: Transcript, after: EntryId? = nil) -> Answer? {
        guard let asked = transcript.entries.lastIndex(where: { entry in
            guard after.map({ entry.id > $0 }) ?? true else { return false }
            if case let .user(text, _) = entry.kind { return text.trimmingCharacters(in: .whitespacesAndNewlines) == prompt }
            return false
        }) else { return nil }
        let after = transcript.entries[(asked + 1)...]
        guard let ended = after.firstIndex(where: { if case .turn = $0.kind { true } else { false } }) else { return nil }
        guard case let .turn(summary) = after[ended].kind, summary.end == .done else { return .noName }
        let reply = after[..<ended]
            .compactMap { if case let .agent(text) = $0.kind { text } else { nil } }
            .joined()
        return name(from: reply).map(Answer.name) ?? Answer.noName
    }

    public enum Answer: Hashable, Sendable {
        case name(String)
        /// The turn ended without one: stopped, refused, failed, or empty.
        case noName
    }

    /// The reply's first line, without the quotes, emphasis, label, or
    /// final period agents add anyway, cut on a word to ``maxLength``.
    static func name(from reply: String) -> String? {
        let line = reply.split(whereSeparator: \.isNewline)
            .map { $0.trimmingCharacters(in: .whitespaces) }
            .first { !$0.isEmpty } ?? ""
        let wrapping = CharacterSet(charactersIn: "\"'`*_#“”‘’ .")
        var name = line.trimmingCharacters(in: wrapping)
        for label in ["Name:", "name:", "Workspace name:"] where name.hasPrefix(label) {
            name = String(name.dropFirst(label.count)).trimmingCharacters(in: wrapping)
        }
        let words = name.split(whereSeparator: \.isWhitespace)
        guard !words.isEmpty else { return nil }
        var kept = String(words[0].prefix(maxLength))
        for word in words.dropFirst() {
            guard kept.count + 1 + word.count <= maxLength else { break }
            kept += " " + word
        }
        return kept
    }
}
