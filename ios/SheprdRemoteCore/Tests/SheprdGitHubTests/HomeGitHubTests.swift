import Foundation
import Testing
@testable import SheprdGitHub
@testable import SheprdRemoteCore

struct HomeGitHubTests {
    private let older = #"""
    {"computer":"Mac","fingerprint":"f","connected":true,"waiting":0,
     "workspaces":[{"id":"w","label":"w","branch":"feature","cwd":"/w","chats":[],"terminals":[]}]}
    """#

    @Test func anOlderHomeNamesNoRepositoryAndLendsNothing() throws {
        let home = try JSONDecoder().decode(Home.self, from: Data(older.utf8))
        #expect(home.githubHosts == nil)
        #expect(home.workspaces[0].github == nil)
        #expect(GitHubLender(device: "d", name: "Mac", home: home, reached: nil).hosts == [])
    }

    @Test func aNewHomeNamesEachWorkspacesRepositoryAndTheAccountsItLends() throws {
        let json = #"""
        {"computer":"Mac","fingerprint":"f","connected":true,"waiting":0,"github":true,
         "github_hosts":[{"host":"github.com","login":"ana"},{"host":"GHE.Example.com","login":"ana-corp"}],
         "workspaces":[
           {"id":"w","label":"w","branch":"feature","cwd":"/w","chats":[],"terminals":[],
            "github":{"host":"github.com","owner":"acme","repo":"app","branch":"feature",
                      "head":"0123456789abcdef0123456789abcdef01234567","head_owner":"ana"}},
           {"id":"d","label":"d","cwd":"/d","chats":[],"terminals":[],
            "github":{"host":"github.com","owner":"acme","repo":"app"}}]}
        """#
        let home = try JSONDecoder().decode(Home.self, from: Data(json.utf8))
        #expect(home.github == true)
        #expect(home.githubHosts == [.init(host: "github.com", login: "ana"), .init(host: "GHE.Example.com", login: "ana-corp")])
        let github = try #require(home.workspaces[0].github)
        #expect(github == .init(
            host: "github.com", owner: "acme", repo: "app", branch: "feature",
            head: "0123456789abcdef0123456789abcdef01234567", headOwner: "ana"
        ))
        #expect(github.headRef?.owner == "ana")
        #expect(github.headRef?.branch == "feature")
        // A detached HEAD has no branch to look a pull request up by.
        #expect(home.workspaces[1].github?.headRef == nil)
        let lender = GitHubLender(device: "d", name: "Mac", home: home, reached: nil)
        #expect(lender.hosts == Set(["github.com", "ghe.example.com"].compactMap(GitHubHost.init)))
        let roundtrip = try JSONDecoder().decode(Home.self, from: JSONEncoder().encode(home))
        #expect(roundtrip == home)
    }

    @Test func aHeadWithoutAForkOwnerIsTheRepositorysOwn() {
        let plain = Home.GitHubRepository(host: "github.com", owner: "acme", repo: "app", branch: "fix")
        #expect(plain.headRef?.owner == "acme")
        let odd = Home.GitHubRepository(host: "github.com", owner: "acme", repo: "app", branch: "fix", head: "0123abc")
        #expect(odd.headRef?.owner == "acme")
        #expect(odd.headRef?.branch == "fix")
    }

    @Test func aHomeWithoutAComputerHomeKeepsItsLeasesUntilOneArrives() {
        #expect(GitHubLender(device: "d", name: "Mac", home: nil, reached: nil).hosts == nil)
    }
}
