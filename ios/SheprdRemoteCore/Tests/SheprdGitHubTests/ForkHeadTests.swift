import Foundation
import Testing
@testable import SheprdGitHub
@testable import SheprdRemoteCore

/// A computer names a fork's checkout by the upstream repository, the
/// branch, its commit, and `head_owner`: forks pushing the same branch name
/// are different pull requests.
struct ForkHeadTests {
    private let transport = FakeTransport()
    private let lenders = FakeLenders()
    private let oid = "0123456789abcdef0123456789abcdef01234567"

    private func reader() -> PullRequestReader {
        PullRequestReader(
            leases: GitHubLeases(store: TokenStore(vault: .memory()), now: { GitHubFixtures.start }),
            api: GitHubAPI(transport: transport.transport, now: { GitHubFixtures.start })
        )
    }

    private var source: Lenders { Lenders(lenders: [GitHubFixtures.lender("mac")], fetch: lenders.fetch) }

    private func fork(_ id: String, _ headOwner: String?) -> Home.Workspace {
        GitHubFixtures.workspace(id: id, github: .init(host: "github.com", owner: "acme", repo: "app", branch: "main", headOwner: headOwner))
    }

    @Test func anOlderComputerWithoutAHeadOwnerReadsAsTheRepositorysOwn() throws {
        let json = #"{"host":"github.com","owner":"acme","repo":"app","branch":"fix","head":"\#(oid)"}"#
        let github = try JSONDecoder().decode(Home.GitHubRepository.self, from: Data(json.utf8))
        #expect(github.headOwner == nil)
        #expect(github.head == oid)
        #expect(github.headRef?.owner == "acme")
        #expect(github.headRef?.branch == "fix")
    }

    @Test func aHeadOwnerNamesTheForkAndRoundTrips() throws {
        let json = #"{"host":"github.com","owner":"acme","repo":"app","branch":"fix","head":"\#(oid)","head_owner":"ana"}"#
        let github = try JSONDecoder().decode(Home.GitHubRepository.self, from: Data(json.utf8))
        #expect(github.headOwner == "ana")
        #expect(github.headRef?.owner == "ana")
        let encoded = try #require(try JSONSerialization.jsonObject(with: JSONEncoder().encode(github)) as? [String: Any])
        #expect(encoded["head_owner"] as? String == "ana")
        #expect(try JSONDecoder().decode(Home.GitHubRepository.self, from: JSONEncoder().encode(github)) == github)
    }

    @Test func aCommitIsNotReadAsAFork() {
        let github = Home.GitHubRepository(host: "github.com", owner: "acme", repo: "app", branch: "fix", head: "ana:fix")
        #expect(github.headRef?.owner == "acme")
    }

    @Test func theListMatchesAWorkspaceToItsForksPullRequestOnly() {
        let pull = GitHubSearchPull(host: GitHubFixtures.host, owner: "acme", repo: "app", number: 51, title: "t", headRefName: "main", headOwner: "fork-owner")
        let places = ComputerWorkspaces.places(of: pull, in: [
            ComputerWorkspaces(device: "m", computer: "M", workspaces: [fork("a", "someone"), fork("b", "Fork-Owner"), fork("c", nil)]),
        ])
        #expect(places.map(\.workspace) == ["b"])
    }

    @Test func twoForksOnOneBranchNameGetTheirOwnBadges() async throws {
        lenders.answer("mac", GitHubFixtures.answer("t1"))
        transport.enqueue(try GitHubFixtures.response("forks"))
        transport.enqueue(try GitHubFixtures.response("forks"))
        let (asked, branches) = DirectBadges.requests([fork("a", "someone"), fork("b", "fork-owner")])
        #expect(asked.count == 2)
        let found = await reader().pullRequests(workspaces: asked, source: source)
        let badges = DirectBadges.answered(found, branches: branches)
        #expect(badges.badge(for: fork("x", "someone")).pullRequest?.number == 50)
        #expect(badges.badge(for: fork("y", "fork-owner")).pullRequest?.number == 51)
        // Each fork is asked with its own head owner: one query each.
        #expect(transport.requests.count == 2)
    }

    @Test func headsAreSplitIntoRoundsThatNameABranchOnce() {
        let rounds = HeadRounds([
            (owner: "acme", branch: "main", oid: nil),
            (owner: "acme", branch: "fix", oid: nil),
            (owner: "ana", branch: "main", oid: oid),
            (owner: "ANA", branch: "main", oid: nil),
        ])
        #expect(rounds.rounds.map(\.branches) == [["main", "fix"], ["main"]])
        #expect(rounds.round(owner: "acme", branch: "fix") == 0)
        #expect(rounds.round(owner: "Ana", branch: "main") == 1)
        #expect(rounds.rounds[1].heads["main"]?.oid == oid)
    }
}
