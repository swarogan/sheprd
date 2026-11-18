import Foundation
@testable import SheprdRemoteCore
import Testing

/// A workspace named by its agent chat: the answer is read only once the
/// turn that answers the prompt ends, and kept short.
struct WorkspaceNamingTests {
    private let prompt = WorkspaceNaming.prompt(current: "worktree/lucky-stone")

    private func transcript(_ kinds: [EntryKind]) -> Transcript {
        var transcript = Transcript(agent: "claude")
        transcript.entries = kinds.enumerated().map { Entry(id: EntryId(UInt64($0.offset)), kind: $0.element) }
        return transcript
    }

    private var ended: EntryKind { turn(.done) }

    private func turn(_ end: TurnEnd) -> EntryKind { .turn(TurnSummary(durationMs: 900, end: end, tokens: nil)) }

    @Test func thePromptNamesTheLimits() {
        #expect(prompt.contains("worktree/lucky-stone"))
        #expect(prompt.contains("\(WorkspaceNaming.maxLength) characters"))
    }

    @Test func waitsForThePrompt() {
        let earlier = transcript([.user(text: "Fix the tests", files: 0), .agent(text: "Done"), ended])
        #expect(WorkspaceNaming.answer(to: prompt, in: earlier) == nil)
    }

    @Test func waitsForTheTurnToEnd() {
        let streaming = transcript([.user(text: prompt, files: 0), .agent(text: "iOS search")])
        #expect(WorkspaceNaming.answer(to: prompt, in: streaming) == nil)
    }

    @Test func readsTheAnswerAfterThePrompt() {
        let answered = transcript([
            .user(text: "Fix the tests", files: 0), .agent(text: "Fixed them all"), ended,
            .user(text: prompt, files: 0), .agent(text: "iOS search "), .agent(text: "first screen"), ended,
        ])
        #expect(WorkspaceNaming.answer(to: prompt, in: answered) == .name("iOS search first screen"))
    }

    @Test func aTurnWithoutTextHasNoName() {
        let refused = transcript([.user(text: prompt, files: 0), ended])
        #expect(WorkspaceNaming.answer(to: prompt, in: refused) == .noName)
    }

    @Test(arguments: [TurnEnd.cancelled, .refused, .failed, .limit, .other])
    func anUnfinishedTurnHasNoNameWhateverItSaid(end: TurnEnd) {
        let stopped = transcript([.user(text: prompt, files: 0), .agent(text: "iOS search"), turn(end)])
        #expect(WorkspaceNaming.answer(to: prompt, in: stopped) == .noName)
    }

    @Test func aRetryWaitsForItsOwnAnswer() {
        // The first ask ended without a name; the retry, same words, is
        // still queued behind the agent's work.
        var queued = transcript([.user(text: prompt, files: 0), turn(.cancelled)])
        let sent = queued.entries.last?.id
        #expect(WorkspaceNaming.answer(to: prompt, in: queued, after: sent) == nil)
        queued.entries += [
            Entry(id: 2, kind: .user(text: prompt, files: 0)),
            Entry(id: 3, kind: .agent(text: "Rename by agent")),
            Entry(id: 4, kind: ended),
        ]
        #expect(WorkspaceNaming.answer(to: prompt, in: queued, after: sent) == .name("Rename by agent"))
    }

    @Test func dropsWhatAgentsAddAnyway() {
        #expect(WorkspaceNaming.name(from: "\"Relay deploy button\".") == "Relay deploy button")
        #expect(WorkspaceNaming.name(from: "**Name:** Pane job rows\n\nThis names the work.") == "Pane job rows")
        #expect(WorkspaceNaming.name(from: "\n  `Rename by agent`  \n") == "Rename by agent")
        #expect(WorkspaceNaming.name(from: "  \n ") == nil)
    }

    @Test func cutsALongAnswerOnAWord() throws {
        let name = try #require(WorkspaceNaming.name(from: "Phone workspace rename through the agent chat"))
        #expect(name == "Phone workspace rename")
        #expect(name.count <= WorkspaceNaming.maxLength)
        let word = try #require(WorkspaceNaming.name(from: String(repeating: "a", count: 60)))
        #expect(word.count == WorkspaceNaming.maxLength)
    }
}
