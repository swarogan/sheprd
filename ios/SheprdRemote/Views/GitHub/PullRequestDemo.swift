#if DEBUG
import SheprdGitHub
import SheprdRemoteCore
import SwiftUI

/// The simulator loop's made-up pull request, for screenshots without a
/// computer or a GitHub account: `-githubDemo header`, `header-computer`,
/// `detail`, or `detail-end`. The answers go through the real GitHub
/// readers from a transport that replays them, so the screens show what
/// the parsing makes of them.
enum PullRequestDemo {
    static var mode: String? { UserDefaults.standard.string(forKey: "githubDemo") }

    static let target = PullRequestTarget(host: GitHubHost("github.com")!, owner: "sheprd-labs", repo: "sheprd", number: 1342)

    /// The detail screen opens scrolled to its end for `detail-end`.
    static var anchor: UnitPoint? { mode == "detail-end" ? .bottom : nil }

    static func reading(_ target: PullRequestTarget) async -> PullRequestReading? {
        guard mode?.hasPrefix("detail") == true, target == Self.target else { return nil }
        let api = GitHubAPI(transport: transport)
        let token = GitHubToken("demo")
        guard let detail = try? await api.pullRequestDetail(host: target.host, owner: target.owner, repo: target.repo, number: target.number, token: token)
        else { return nil }
        let files = try? await api.pullRequestFiles(host: target.host, owner: target.owner, repo: target.repo, number: target.number, token: token)
        return PullRequestReading(detail: detail, files: files)
    }

    static func pullRequest() async -> GitHubPullRequest? {
        let api = GitHubAPI(transport: transport)
        let found = try? await api.pullRequests(
            host: target.host, owner: target.owner, repo: target.repo, branches: ["phone-github-reads"],
            headOwner: { _ in "sheprd-labs" }, token: GitHubToken("demo")
        )
        return (try? found?["phone-github-reads"]?.get()) ?? nil
    }

    static let workspace: Home.Workspace? = {
        let json = """
        {"id":"w","label":"sheprd","branch":"phone-github-reads","cwd":"/demo","chats":[],"terminals":[],
         "pull_request":{"number":1342,"url":"https://github.com/sheprd-labs/sheprd/pull/1342","tone":"attention"},
         "github":{"host":"github.com","owner":"sheprd-labs","repo":"sheprd","branch":"phone-github-reads"}}
        """
        return try? JSONDecoder().decode(Home.Workspace.self, from: Data(json.utf8))
    }()

    static let checks = PullRequestChecks(number: 1342, checks: [
        PullRequestCheck(name: "clippy (linux, all)", state: .failed, job: 1, url: "https://github.com/sheprd-labs/sheprd/actions/runs/1/job/1"),
        PullRequestCheck(name: "test (windows, default)", state: .failed, job: 2, url: "https://github.com/sheprd-labs/sheprd/actions/runs/1/job/2"),
        PullRequestCheck(name: "ios-build", state: .pending),
    ] + ["fmt", "test (linux, default)", "test (macos, all)", "commits", "check-file-size"].map {
        PullRequestCheck(name: $0, state: .passed)
    })

    static let files: [ChangedFile] = {
        let json = """
        [{"path":"ios/SheprdRemoteCore/Sources/GitHub/PullRequestQuery.swift","status":"added","added":214,"removed":0,"shown":"text"},
         {"path":"ios/SheprdRemoteCore/Sources/GitHub/Lease.swift","status":"modified","added":132,"removed":4,"shown":"text"},
         {"path":"ios/SheprdRemote/Views/ChangesScreen.swift","status":"modified","added":88,"removed":31,"shown":"text"},
         {"path":"crates/sheprd-remote-control/src/server/github.rs","status":"modified","added":176,"removed":12,"shown":"text"},
         {"path":"docs/ios-github-plan.md","status":"modified","added":9,"removed":3,"shown":"text"}]
        """
        return (try? JSONDecoder().decode([ChangedFile].self, from: Data(json.utf8))) ?? []
    }()

    private static let transport: GitHubTransport = { request, _ in
        let path = request.url?.path() ?? ""
        let query = request.url?.query() ?? ""
        let body: Any
        if path.hasSuffix("/files") {
            body = query.contains("page=1") ? patches : []
        } else if String(decoding: request.httpBody ?? Data(), as: UTF8.self).contains("timelineItems") {
            body = detail
        } else {
            body = ["data": ["repository": ["nameWithOwner": "sheprd-labs/sheprd", "b0": ["nodes": [summary]]]]]
        }
        return GitHubResponse(status: 200, body: try JSONSerialization.data(withJSONObject: body))
    }

    private static func ago(_ seconds: TimeInterval) -> String {
        Date.now.addingTimeInterval(-seconds).formatted(.iso8601)
    }

    private static let title = "Read pull requests on the phone with a lent GitHub sign-in"

    private static var summary: [String: Any] {
        [
            "number": 1342, "title": title, "url": "https://github.com/sheprd-labs/sheprd/pull/1342",
            "state": "OPEN", "isDraft": false, "reviewDecision": "CHANGES_REQUESTED", "mergeable": "MERGEABLE",
            "mergeStateStatus": "BLOCKED", "headRefName": "phone-github-reads", "headRefOid": String(repeating: "a", count: 40),
            "author": ["login": "octocat"], "headRepositoryOwner": ["login": "sheprd-labs"],
            "commits": ["nodes": [["commit": ["statusCheckRollup": ["contexts": ["pageInfo": ["hasNextPage": false], "nodes":
                checks.checks.enumerated().map { index, check -> [String: Any] in
                    let conclusion = switch check.state {
                    case .failed: "FAILURE"
                    case .passed: "SUCCESS"
                    default: "NEUTRAL"
                    }
                    return [
                        "__typename": "CheckRun", "name": check.name, "status": check.state == .pending ? "IN_PROGRESS" : "COMPLETED",
                        "conclusion": conclusion, "databaseId": index + 1,
                        "detailsUrl": "https://github.com/sheprd-labs/sheprd/actions/runs/1/job/\(index + 1)",
                    ]
                },
            ]]]]]],
        ]
    }

    private static var detail: [String: Any] {
        let body = """
        The phone now asks GitHub itself, with a sign-in a paired computer lends it.

        - One GraphQL query per repository resolves the branch to its pull request.
        - A failing job's log comes over REST, cut to **8 KiB** as `check_log` does.
        - With no lease, the phone asks the computer as before.

        Closes #1287.
        """
        let user = { (login: String) in ["__typename": "User", "login": login] }
        return ["data": ["repository": ["nameWithOwner": "sheprd-labs/sheprd", "pullRequest": [
            "number": 1342, "title": title, "url": "https://github.com/sheprd-labs/sheprd/pull/1342", "state": "OPEN",
            "isDraft": false, "reviewDecision": "CHANGES_REQUESTED", "body": body, "createdAt": ago(5 * 3600),
            "additions": 619, "deletions": 50, "changedFiles": 5, "baseRefName": "main", "headRefName": "phone-github-reads",
            "author": ["login": "octocat"],
            "reviewRequests": ["nodes": [["requestedReviewer": ["__typename": "Team", "slug": "ios", "organization": ["login": "sheprd-labs"]]]]],
            "latestReviews": ["nodes": [
                ["state": "APPROVED", "author": ["login": "hubot"]],
                ["state": "CHANGES_REQUESTED", "author": ["login": "monalisa"]],
            ]],
            "timelineItems": ["totalCount": 6, "pageInfo": ["hasPreviousPage": false], "nodes": [
                ["__typename": "ReviewRequestedEvent", "id": "RR", "actor": ["login": "octocat"], "createdAt": ago(4.9 * 3600),
                 "requestedReviewer": user("hubot")],
                ["__typename": "PullRequestReview", "id": "R1", "author": ["login": "hubot"], "submittedAt": ago(2 * 3600),
                 "state": "APPROVED", "body": "Lovely. The 60 s refresh only runs while the screen is visible, right?",
                 "comments": ["totalCount": 0, "nodes": []]],
                ["__typename": "PullRequestReview", "id": "R2", "author": ["login": "monalisa"], "submittedAt": ago(3600),
                 "state": "CHANGES_REQUESTED", "body": "",
                 "comments": ["totalCount": 1, "nodes": [["path": "ios/SheprdRemoteCore/Sources/GitHub/Lease.swift", "line": 88,
                     "body": "A 401 should drop the lease before asking again, or two screens race to refetch it."]]]],
                ["__typename": "PullRequestCommit", "id": "C1", "commit": ["abbreviatedOid": "3f2a91c",
                 "messageHeadline": "fix(ios): fetch one lease at a time", "committedDate": ago(45 * 60), "author": ["user": ["login": "octocat"]]]],
                ["__typename": "IssueComment", "id": "IC1", "author": ["login": "octocat"], "createdAt": ago(40 * 60),
                 "body": "Good catch, fixed in 3f2a91c. One fetch at a time now."],
            ]],
        ]]]]
    }

    private static var patches: [[String: Any]] {
        files.map { file in
            [
                "filename": file.path, "status": file.status == .added ? "added" : "modified",
                "additions": file.added ?? 0, "deletions": file.removed ?? 0, "changes": (file.added ?? 0) + (file.removed ?? 0),
                "patch": "@@ -84,6 +84,9 @@ public actor GitHubLeases {\n     guard lease.isValid else {\n-        return try await fetch(host)\n+        // One fetch at a time per host.\n+        if let pending = inFlight[host] { return try await pending.value }\n+        return try await refetch(host)\n     }\n }",
            ]
        }
    }
}

/// What `-githubDemo` shows over the app.
struct PullRequestDemoRoot: View {
    @State private var pullRequest: GitHubPullRequest?

    var body: some View {
        switch PullRequestDemo.mode {
        case "header", "header-computer":
            NavigationStack { header }.task {
                if PullRequestDemo.mode == "header" { pullRequest = await PullRequestDemo.pullRequest() }
            }
        case "detail", "detail-end":
            NavigationStack {
                PullRequestDetailScreen(target: PullRequestDemo.target, summary: PullRequestSummary(number: 1342))
            }
        default:
            EmptyView()
        }
    }

    private var header: some View {
        List {
            Section {
                Picker("Compare", selection: .constant(ChangeScope.branch)) {
                    Text("Uncommitted").tag(ChangeScope.uncommitted)
                    Text("Branch").tag(ChangeScope.branch)
                }
                .pickerStyle(.segmented)
                .listRowBackground(Color.clear)
                .listRowInsets(EdgeInsets())
            }
            PullRequestHeader(
                checks: PullRequestDemo.checks, pullRequest: pullRequest, workspace: PullRequestDemo.workspace,
                origin: PullRequestDemo.mode == "header" ? .github : .computer("MacBook Pro")
            ) { _ in }
            Section("5 files against main") {
                ForEach(PullRequestDemo.files) { FileRow(file: $0) }
            }
        }
        .listSectionSpacing(14)
        .navigationTitle("Changes")
        .navigationBarTitleDisplayMode(.inline)
    }
}
#endif
