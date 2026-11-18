import Foundation
import Testing
@testable import SheprdGitHub
@testable import SheprdRemoteCore

struct PullRequestMappingTests {
    private let host = GitHubFixtures.host

    private func parse(_ name: String, branches: [String] = ["feature"], headOwner: String = "acme", headers: [String: String] = [:]) throws -> [String: Result<GitHubPullRequest?, GitHubError>] {
        try PullRequestAnswer.parse(
            GitHubFixtures.response(name, headers: headers),
            query: PullRequestQuery(owner: "acme", repo: "app", branches: branches),
            headOwner: { _ in headOwner }, host: host, now: GitHubFixtures.start
        )
    }

    @Test func anOpenPullRequestWithPassingChecksIsReady() throws {
        let pr = try #require(try parse("open_pr")["feature"]?.get())
        #expect(pr.number == 42)
        #expect(pr.title == "Add login")
        #expect(pr.state == .open)
        #expect(pr.reviewDecision == .approved)
        #expect(pr.mergeable == .mergeable)
        #expect(pr.author == "ana")
        #expect(pr.headRefOid == "0123456789abcdef0123456789abcdef01234567")
        #expect(pr.tone == .ready)
        #expect(pr.badge == Home.PullRequest(number: 42, url: "https://github.com/acme/app/pull/42", tone: .ready))
        #expect(pr.checkList == PullRequestChecks(number: 42, checks: [
            PullRequestCheck(name: "test", state: .passed, job: 70, url: "https://github.com/acme/app/actions/runs/7/job/70"),
        ]))
    }

    @Test func aDraftIsMuted() throws {
        let pr = try #require(try parse("draft")["feature"]?.get())
        #expect(pr.isDraft)
        #expect(pr.reviewDecision == .none)
        #expect(pr.tone == .other)
        #expect(pr.checks.map(\.state) == [.pending])
    }

    @Test func aBranchWithoutAPullRequestHasNoneAndAReusedOneTakesTheOpenOne() throws {
        let found = try parse("no_pr", branches: ["feature", "reused"])
        #expect(try found["feature"]?.get() == .some(nil))
        let reused = try #require(try found["reused"]?.get())
        #expect(reused.number == 12)
        #expect(reused.tone == .waiting)
        #expect(reused.checks.isEmpty)
    }

    @Test func mixedChecksMapToTheComputersStatesAndOnlyActionsJobsHaveLogs() throws {
        let pr = try #require(try parse("mixed_checks")["feature"]?.get())
        #expect(pr.moreChecks)
        #expect(pr.tone == .attention)
        #expect(pr.checks == [
            PullRequestCheck(name: "lint", state: .failed, job: 80, url: "https://github.com/acme/app/actions/runs/8/job/80"),
            PullRequestCheck(name: "build", state: .pending, job: 81, url: "https://github.com/acme/app/actions/runs/8/job/81"),
            // Another app's check run has an id, but no log GitHub serves.
            PullRequestCheck(name: "deploy-preview", state: .failed, job: nil, url: "https://vercel.example/acme/app"),
            PullRequestCheck(name: "optional", state: .skipped),
            PullRequestCheck(name: "docs", state: .skipped),
            // A plain address is not offered.
            PullRequestCheck(name: "ci/legacy", state: .failed),
            PullRequestCheck(name: "ci/pending", state: .pending, url: "https://ci.example/2"),
        ])
        #expect(pr.checkList.open.map(\.name) == ["lint", "deploy-preview", "ci/legacy", "build", "ci/pending"])
    }

    @Test func statusContextsReportThroughTheirState() throws {
        let pr = try #require(try parse("status_context")["feature"]?.get())
        #expect(pr.checks == [
            PullRequestCheck(name: "ci/build", state: .passed, url: "https://ci.example/build"),
            PullRequestCheck(name: "ci/deploy", state: .pending),
        ])
        #expect(pr.tone == .waiting)
    }

    @Test func aForksBranchIsTheOneItsHeadOwnerOpened() throws {
        let pr = try #require(try parse("forks", branches: ["main"], headOwner: "fork-owner")["main"]?.get())
        #expect(pr.number == 51)
        #expect(pr.tone == .held)
        #expect(throws: GitHubError.ambiguous) { try parse("forks", branches: ["main"], headOwner: "nobody")["main"]?.get() }
    }

    @Test func aMissingRepositoryIsNotFoundAndAnSSOChallengeIsSSO() throws {
        #expect(throws: GitHubError.notFound) { try parse("not_found") }
        #expect(throws: GitHubError.forbidden) { try parse("sso") }
        #expect(throws: GitHubError.ssoRequired) { try parse("sso", headers: ["X-GitHub-SSO": "required; url=https://github.com/orgs/acme/sso?authorization_request=x"]) }
    }

    @Test func toneFollowsTheComputersRules() {
        var pr = GitHubPullRequest(number: 1, title: "", url: "", state: .open, isDraft: false, reviewDecision: .none, mergeable: .unknown, mergeState: .clean, author: nil, headRefOid: "", checks: [], moreChecks: false)
        #expect(pr.tone == .ready)
        pr.mergeState = .unstable
        pr.checks = [PullRequestCheck(name: "a", state: .pending)]
        #expect(pr.tone == .waiting)
        pr.checks = []
        #expect(pr.tone == .attention)
        pr.mergeState = .behind
        #expect(pr.tone == .held)
        pr.reviewDecision = .changesRequested
        #expect(pr.tone == .attention)
        pr.state = .merged
        #expect(pr.tone == .merged)
        pr.state = .closed
        #expect(pr.tone == .attention)
    }

    @Test func branchesAreBatchedWithAliasesAndVariables() throws {
        let branches = (0..<45).map { "branch-\($0)" } + ["branch-0"]
        let batches = PullRequestQuery.batches(owner: "acme", repo: "app", branches: branches)
        #expect(batches.map(\.branches.count) == [20, 20, 5])
        let query = batches[0]
        #expect(query.text.contains("b19: pullRequests(first: 5, headRefName: $b19"))
        #expect(!query.text.contains("branch-0"))
        let body = try #require(try JSONSerialization.jsonObject(with: query.body) as? [String: Any])
        let variables = try #require(body["variables"] as? [String: String])
        #expect(variables["owner"] == "acme")
        #expect(variables["b0"] == "branch-0")
        #expect(variables.count == 22)
    }
}
