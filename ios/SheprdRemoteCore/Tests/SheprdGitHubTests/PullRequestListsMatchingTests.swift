import Testing
@testable import SheprdGitHub

/// The Pull Requests tab's search, over the lists already read.
struct PullRequestListsMatchingTests {
    private static func lists() throws -> PullRequestLists {
        let host = try #require(GitHubHost("github.com"))
        let search = GitHubSearchPull(host: host, owner: "sheprd-labs", repo: "sheprd", number: 1349, title: "Search Home",
                                      author: "penso", headRefName: "worktree/lucky-stone")
        let relay = GitHubSearchPull(host: host, owner: "octo-org", repo: "infra", number: 902, title: "Bump relay droplets",
                                     author: "dependabot", headRefName: "deps/relay")
        return PullRequestLists(mine: [search, relay], reviewRequested: [relay], mineTotal: 9, reviewRequestedTotal: 3)
    }

    private static func titles(_ lists: PullRequestLists) -> [String] {
        (lists.reviewRequested + lists.mine).map(\.title)
    }

    @Test func anEmptyQueryKeepsEverythingAndItsTotals() throws {
        let lists = try Self.lists()
        #expect(lists.matching("  ") == lists)
    }

    @Test func everyWordMustMatchSomeField() throws {
        let lists = try Self.lists()
        #expect(Self.titles(lists.matching("SHEPRD home")) == ["Search Home"])
        #expect(Self.titles(lists.matching("dependabot relay")) == ["Bump relay droplets", "Bump relay droplets"])
        #expect(lists.matching("sheprd relay").isEmpty)
    }

    @Test func aNumberMatchesWithOrWithoutItsHash() throws {
        let lists = try Self.lists()
        #expect(Self.titles(lists.matching("#1349")) == ["Search Home"])
        #expect(Self.titles(lists.matching("lucky-stone")) == ["Search Home"])
    }

    @Test func totalsBecomeWhatMatched() throws {
        let found = try Self.lists().matching("relay")
        #expect(found.mineTotal == 1)
        #expect(found.reviewRequestedTotal == 1)
    }
}
