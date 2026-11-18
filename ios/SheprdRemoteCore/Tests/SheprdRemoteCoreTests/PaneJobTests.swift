import Foundation
@testable import SheprdRemoteCore
import Testing

/// What a terminal pane runs in front of its shell, as the workspace
/// screen's row shows it instead of the folder.
struct PaneJobTests {
    private func pane(_ json: String) throws -> Home.TerminalRow.Pane {
        try JSONDecoder().decode(Home.TerminalRow.Pane.self, from: Data(json.utf8))
    }

    @Test func readsARunningCommand() throws {
        let running = try pane(#"{"pane":"p1","focused":true,"job":{"state":"running","command":"cargo test"}}"#)
        #expect(running.job == .running(command: "cargo test"))
    }

    @Test func readsAnIdleShell() throws {
        #expect(try pane(#"{"pane":"p1","focused":true,"job":{"state":"idle"}}"#).job == .idle)
    }

    @Test func anOlderComputerSaysNothing() throws {
        #expect(try pane(#"{"pane":"p1","focused":true}"#).job == nil)
    }

    @Test func aNewerStateStillDecodes() throws {
        #expect(try pane(#"{"pane":"p1","focused":true,"job":{"state":"stopped"}}"#).job == .other)
    }

    @Test func roundTripsThroughTheCache() throws {
        let original = try pane(#"{"pane":"p1","focused":true,"job":{"state":"running","command":"htop"}}"#)
        let again = try JSONDecoder().decode(Home.TerminalRow.Pane.self, from: JSONEncoder().encode(original))
        #expect(again == original)
    }
}

/// A chat its agent has not titled is named by its first prompt.
struct FirstPromptTests {
    @Test func readsTheFirstPrompt() throws {
        let json = #"{"connected":true,"status":{"state":"idle"},"questions":0,"first_prompt":"Fix the login flow"}"#
        let summary = try JSONDecoder().decode(Home.Summary.self, from: Data(json.utf8))
        #expect(summary.firstPrompt == "Fix the login flow")
    }

    @Test func anOlderComputerSendsNone() throws {
        let json = #"{"connected":true,"status":{"state":"idle"},"questions":0}"#
        #expect(try JSONDecoder().decode(Home.Summary.self, from: Data(json.utf8)).firstPrompt == nil)
    }
}
