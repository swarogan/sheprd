import Foundation
import Testing
@testable import SheprdGitHub
@testable import SheprdRemoteCore

struct PullRequestDetailMappingTests {
    private func parse(_ name: String) throws -> PullRequestDetail {
        try PullRequestDetailAnswer.parse(try GitHubFixtures.response(name), now: GitHubFixtures.start)
    }

    @Test func theSummaryIsReadAndCleaned() throws {
        let detail = try parse("pr_detail")
        #expect(detail.number == 42)
        #expect(detail.title == "Read pull requests on the phone")
        #expect(detail.url == "https://github.com/acme/app/pull/42")
        #expect(detail.repository == "acme/app")
        #expect(detail.state == .open)
        #expect(detail.reviewDecision == .changesRequested)
        #expect(detail.author == "ana")
        #expect((detail.baseRef, detail.headRef) == ("main", "feature"))
        #expect((detail.additions, detail.deletions, detail.changedFiles) == (619, 50, 5))
        #expect(detail.createdAt == (try? Date("2026-11-14T09:00:00Z", strategy: .iso8601)))
    }

    @Test func theDescriptionKeepsItsLinesButNoOtherControls() throws {
        let body = try parse("pr_detail").body
        #expect(body == "The phone asks GitHub itself.\n\n- One query\n- One log\n\nCloses #12.")
    }

    @Test func reviewersAreLatestVerdictsThenOpenRequests() throws {
        let reviewers = try parse("pr_detail").reviewers
        #expect(reviewers == [
            .init(name: "hubot", verdict: .approved),
            .init(name: "mona", verdict: .changesRequested),
            // Asked again after commenting: waiting on them once more.
            .init(name: "bo", verdict: nil),
            .init(name: "acme/ios", isTeam: true),
        ])
    }

    @Test func theTimelineReadsInOrderWithFileAndLineOnReviewComments() throws {
        let timeline = try parse("pr_detail").timeline
        #expect(timeline.map(\.id) == ["C1", "RR1", "R1", "R2", "F1", "IC1"])
        #expect(timeline[0].kind == .commit(oid: "3f2a91c", headline: "feat: read it"))
        #expect(timeline[0].actor == "ana")
        #expect(timeline[1].kind == .reviewRequested(reviewer: "hubot"))
        #expect(timeline[2].kind == .review(verdict: .approved, body: "Lovely.", comments: [], more: 0))
        #expect(timeline[2].date == (try? Date("2026-11-14T11:01:00Z", strategy: .iso8601)))
        #expect(timeline[3].kind == .review(verdict: .changesRequested, body: "", comments: [
            .init(path: "Sources/Lease.swift", line: 88, body: "A 401 should drop the lease."),
            // An outdated comment keeps the line it was written on.
            .init(path: "Sources/Old.swift", line: 12, body: "Outdated."),
        ], more: 0))
        #expect(timeline[4].kind == .forcePushed(oid: "9e8d7c6"))
        #expect(timeline[5].kind == .comment(body: "Good catch, fixed in 9e8d7c6."))
        #expect(timeline[5].actor == nil)
        #expect(try parse("pr_detail").earlierItems == 0)
    }

    @Test func anEmptyPullRequestHasNoBodyReviewersOrUnsafeLink() throws {
        let detail = try parse("pr_detail_empty")
        #expect(detail.body.isEmpty)
        #expect(detail.reviewers.isEmpty)
        #expect(detail.timeline.isEmpty)
        #expect(detail.author == nil)
        #expect(detail.state == .merged)
        #expect(detail.reviewDecision == .none)
        // Only `https` pages are offered.
        #expect(detail.url.isEmpty)
    }

    @Test func manyItemsAreBoundedAndCounted() throws {
        let comments = (0..<25).map { ["path": "f\($0).swift", "line": $0 + 1, "body": "note \($0)"] as [String: Any] }
        var nodes: [[String: Any]] = (0..<49).map {
            ["__typename": "IssueComment", "id": "IC\($0)", "author": ["login": "ana"], "createdAt": "2026-11-14T09:00:00Z", "body": "c\($0)"]
        }
        nodes.append([
            "__typename": "PullRequestReview", "id": "R", "author": ["login": "mona"], "state": "COMMENTED", "body": "",
            "comments": ["totalCount": 25, "nodes": comments],
        ])
        let reviewers = (0..<30).map { ["requestedReviewer": ["__typename": "User", "login": "r\($0)"]] }
        let pull: [String: Any] = [
            "number": 1, "title": "Big", "url": "https://github.com/acme/app/pull/1", "state": "OPEN", "isDraft": true,
            "body": String(repeating: "x", count: PullRequestDetailQuery.bodyLimit + 10),
            "reviewRequests": ["nodes": reviewers], "latestReviews": ["nodes": []],
            "timelineItems": ["totalCount": 180, "pageInfo": ["hasPreviousPage": true], "nodes": nodes],
        ]
        let body = try JSONSerialization.data(withJSONObject: ["data": ["repository": ["nameWithOwner": "acme/app", "pullRequest": pull]]])
        let detail = try PullRequestDetailAnswer.parse(GitHubResponse(status: 200, body: body), now: GitHubFixtures.start)
        #expect(detail.isDraft)
        #expect(detail.earlierItems == 130)
        #expect(detail.timeline.count == 50)
        #expect(detail.reviewers.count == PullRequestDetailQuery.reviewerLimit)
        #expect(detail.body.hasSuffix("\n\n…"))
        #expect(detail.body.count == PullRequestDetailQuery.bodyLimit + 3)
        guard case let .review(_, _, notes, more) = try #require(detail.timeline.last).kind else {
            Issue.record("not a review")
            return
        }
        #expect(notes.count == PullRequestDetailQuery.reviewCommentLimit)
        #expect(more == 15)
    }

    @Test func theQuerySendsNamesAsVariablesWithBoundedLists() throws {
        let query = PullRequestDetailQuery(owner: "acme\") { x", repo: "app", number: 42)
        let sent = try #require(try JSONSerialization.jsonObject(with: query.body) as? [String: Any])
        let variables = try #require(sent["variables"] as? [String: Any])
        #expect(variables["owner"] as? String == "acme\") { x")
        #expect(variables["number"] as? Int == 42)
        let text = try #require(sent["query"] as? String)
        #expect(!text.contains("acme"))
        #expect(text.contains("timelineItems(last: 50"))
        #expect(text.contains("comments(first: 10)"))
    }
}
