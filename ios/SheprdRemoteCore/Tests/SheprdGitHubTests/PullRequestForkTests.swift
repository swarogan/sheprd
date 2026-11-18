import Foundation
import Testing
@testable import SheprdGitHub
@testable import SheprdRemoteCore

/// A fork's checkout: the computer names the upstream repository and the
/// checkout's commit, and the pull request's head belongs to the fork.
struct PullRequestForkTests {
    private let oid = "0123456789abcdef0123456789abcdef01234567"

    private func parse(_ name: String, headOid: String?) throws -> GitHubPullRequest? {
        try PullRequestAnswer.parse(
            GitHubFixtures.response(name),
            query: PullRequestQuery(owner: "upstream", repo: "app", branches: ["feature"]),
            headOwner: { _ in "upstream" }, headOid: { _ in headOid },
            host: GitHubFixtures.host, now: GitHubFixtures.start
        )["feature"]?.get()
    }

    @Test func theCheckoutsCommitFindsAForksPullRequest() throws {
        #expect(try parse("open_pr", headOid: oid)?.number == 42)
    }

    @Test func anotherOwnersHeadAtAnotherCommitIsNotTheBranchs() throws {
        #expect(throws: GitHubError.malformed) { try parse("open_pr", headOid: String(repeating: "f", count: 40)) }
        #expect(throws: GitHubError.malformed) { try parse("open_pr", headOid: nil) }
    }

    @Test func twoForksAtTheSameCommitStayAmbiguous() throws {
        #expect(throws: GitHubError.ambiguous) {
            try PullRequestAnswer.parse(
                GitHubFixtures.response("forks"),
                query: PullRequestQuery(owner: "acme", repo: "app", branches: ["main"]),
                headOwner: { _ in "nobody" }, headOid: { _ in oid },
                host: GitHubFixtures.host, now: GitHubFixtures.start
            )["main"]?.get()
        }
    }
}

/// A reused branch: the owner's older pull request ended at the
/// checkout's commit, and a newer one is open.
struct PullRequestReusedBranchTests {
    @Test func theOwnersOpenPullRequestWinsOverAnOlderOneAtTheCheckoutsCommit() throws {
        let old = "1111111111111111111111111111111111111111"
        var json = try #require(try JSONSerialization.jsonObject(with: GitHubFixtures.data("forks")) as? [String: Any])
        var data = try #require(json["data"] as? [String: Any])
        var repository = try #require(data["repository"] as? [String: Any])
        var branch = try #require(repository["b0"] as? [String: Any])
        var nodes = try #require(branch["nodes"] as? [[String: Any]])
        nodes[0]["headRepositoryOwner"] = ["login": "acme"]
        nodes[0]["state"] = "CLOSED"
        nodes[0]["headRefOid"] = old
        nodes[1]["headRepositoryOwner"] = ["login": "acme"]
        branch["nodes"] = nodes
        repository["b0"] = branch
        data["repository"] = repository
        json["data"] = data
        let response = GitHubResponse(status: 200, headers: [:], body: try JSONSerialization.data(withJSONObject: json))
        let found = try PullRequestAnswer.parse(
            response,
            query: PullRequestQuery(owner: "acme", repo: "app", branches: ["main"]),
            headOwner: { _ in "acme" }, headOid: { _ in old },
            host: GitHubFixtures.host, now: GitHubFixtures.start
        )
        #expect(try found["main"]?.get()?.number == 51)
    }
}
