import Foundation
import Testing
@testable import SheprdGitHub
@testable import SheprdRemoteCore

/// A listed pull request finds the workspaces whose branch carries it, on
/// any paired computer.
struct WorkspaceMatchTests {
    private let pull = GitHubSearchPull(
        host: GitHubFixtures.host, owner: "Sheprd-Labs", repo: "Sheprd", number: 1342,
        title: "t", headRefName: "feat/x", headOwner: "sheprd-labs"
    )

    private func computer(_ name: String, _ workspaces: Home.Workspace...) -> ComputerWorkspaces {
        ComputerWorkspaces(device: name.lowercased(), computer: name, workspaces: workspaces)
    }

    private func workspace(_ id: String, host: String = "github.com", owner: String = "sheprd-labs", repo: String = "sheprd", branch: String? = "feat/x", headOwner: String? = nil) -> Home.Workspace {
        GitHubFixtures.workspace(id: id, github: .init(host: host, owner: owner, repo: repo, branch: branch, headOwner: headOwner))
    }

    @Test func aWorkspaceOnTheBranchMatchesAcrossComputers() {
        let places = ComputerWorkspaces.places(of: pull, in: [
            computer("MacBook Pro", workspace("a"), workspace("b", branch: "main")),
            computer("studio", workspace("c")),
        ])
        #expect(places == [
            WorkspacePlace(device: "macbook pro", computer: "MacBook Pro", workspace: "a"),
            WorkspacePlace(device: "studio", computer: "studio", workspace: "c"),
        ])
    }

    @Test func ownersAndRepositoriesCompareWithoutCaseButBranchesDoNot() {
        #expect(ComputerWorkspaces.places(of: pull, in: [computer("m", workspace("a", owner: "SHEPRD-LABS", repo: "sheprd"))]).count == 1)
        #expect(ComputerWorkspaces.places(of: pull, in: [computer("m", workspace("a", branch: "Feat/X"))]).isEmpty)
    }

    @Test func anotherHostOrRepositoryDoesNotMatch() {
        #expect(ComputerWorkspaces.places(of: pull, in: [
            computer("m", workspace("a", host: "acme.ghe.com"), workspace("b", repo: "sheprd-apns"), workspace("c", owner: "fork")),
        ]).isEmpty)
    }

    @Test func aDetachedHeadOrNoRepositoryDoesNotMatch() {
        let bare = GitHubFixtures.workspace(id: "bare", github: nil)
        #expect(ComputerWorkspaces.places(of: pull, in: [computer("m", workspace("a", branch: nil), bare)]).isEmpty)
    }

    @Test func aForksBranchMatchesItsHeadOwner() {
        var fork = pull
        fork.headOwner = "octocat"
        // The computer names the upstream repository and the fork's owner.
        let forked = workspace("a", headOwner: "octocat")
        #expect(ComputerWorkspaces.places(of: fork, in: [computer("m", forked)]).count == 1)
        // The same branch name on upstream itself is another pull request.
        #expect(ComputerWorkspaces.places(of: fork, in: [computer("m", workspace("b"))]).isEmpty)
    }

    @Test func aDeletedForkMatchesTheRepositorysOwner() {
        var gone = pull
        gone.headOwner = nil
        #expect(ComputerWorkspaces.places(of: gone, in: [computer("m", workspace("a"))]).count == 1)
    }
}
