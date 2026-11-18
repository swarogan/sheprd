import Foundation
import Testing
@testable import SheprdRemoteCore

/// The agent lists narrowed to one repository or one workspace.
struct AgentScopeTests {
    private static let repo = Home.Worktree(key: "/a/.git", label: "repo", linked: false)
    private static let otherCheckout = Home.Worktree(key: "/b/.git", label: "repo", linked: true)

    private static func workspace(_ id: String, worktree: Home.Worktree? = nil, agent: Bool = true) -> Home.Workspace {
        let chats = agent ? [Home.ChatRow(pane: "p-\(id)", tab: "t", title: "chat", agent: "claude", status: .idle)] : []
        return Home.Workspace(id: id, label: id, branch: nil, cwd: "/\(id)", worktree: worktree, session: nil, pullRequest: nil, chats: chats, terminals: [])
    }

    private static let homes: [(device: String, home: Home)] = [
        ("one", RepositoriesTests.home([workspace("w1", worktree: repo), workspace("loose"), workspace("quiet", worktree: repo, agent: false)])),
        ("two", RepositoriesTests.home([workspace("w1", worktree: otherCheckout), workspace("Alpha")])),
    ]

    @Test func aRepositoryCoversItsCheckoutsOnEveryComputer() {
        let scope = AgentScope.repository("repo")
        #expect(scope.contains(Self.workspace("w1", worktree: Self.repo), on: "one"))
        #expect(scope.contains(Self.workspace("w9", worktree: Self.otherCheckout), on: "two"))
        #expect(!scope.contains(Self.workspace("repo-less"), on: "one"))
    }

    @Test func aWorkspaceIsOnlyThatOneOnItsComputer() {
        let scope = AgentScope.workspace(device: "one", id: "w1")
        #expect(scope.contains(Self.workspace("w1"), on: "one"))
        #expect(!scope.contains(Self.workspace("w1"), on: "two"))
        #expect(!scope.contains(Self.workspace("w2"), on: "one"))
    }

    @Test func offersRepositoriesWithAgentsAlphabeticallyAcrossComputers() {
        let choices = AgentScope.choices(among: Self.homes)
        #expect(choices.map(\.name) == ["Alpha", "loose", "repo"])
        let repo = choices.first { $0.name == "repo" }
        // The checkout with no agent is left out.
        #expect(repo?.places.map(\.scope) == [.workspace(device: "one", id: "w1"), .workspace(device: "two", id: "w1")])
    }

    @Test func dropsAScopeNoWorkspaceFallsInAnyMore() {
        #expect(AgentScope.active(nil, among: Self.homes) == nil)
        #expect(AgentScope.active(.repository("repo"), among: Self.homes) == .repository("repo"))
        #expect(AgentScope.active(.repository("gone"), among: Self.homes) == nil)
        #expect(AgentScope.active(.workspace(device: "two", id: "Alpha"), among: Self.homes) != nil)
        #expect(AgentScope.active(.workspace(device: "one", id: "Alpha"), among: Self.homes) == nil)
    }

    @Test func survivesBeingSaved() throws {
        for scope in [AgentScope.repository("repo"), .workspace(device: "one", id: "w1")] {
            let data = try JSONEncoder().encode(scope)
            #expect(try JSONDecoder().decode(AgentScope.self, from: data) == scope)
        }
    }
}
