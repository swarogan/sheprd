import Foundation
import Testing
@testable import SheprdGitHub
@testable import SheprdRemoteCore

struct PullRequestFilesTests {
    private let host = GitHubFixtures.host
    private let token = GitHubToken("ghu_secret")

    static func page(_ count: Int, from start: Int = 0, patch: String? = "@@ -1,2 +1,2 @@\n a\n-b\n+c") throws -> GitHubResponse {
        let files: [[String: Any]] = (start..<start + count).map { index in
            var file: [String: Any] = ["filename": "f\(index).swift", "status": "modified", "additions": 1, "deletions": 1, "changes": 2]
            if let patch { file["patch"] = patch }
            return file
        }
        return GitHubResponse(status: 200, body: try JSONSerialization.data(withJSONObject: files))
    }

    private func read(_ transport: FakeTransport) async throws -> PullRequestFiles {
        let api = GitHubAPI(transport: transport.transport, now: { GitHubFixtures.start })
        return try await api.pullRequestFiles(host: host, owner: "acme", repo: "app", number: 42, token: token)
    }

    @Test func aShortPageIsTheLast() async throws {
        let transport = FakeTransport([try Self.page(100), try Self.page(3, from: 100)])
        let files = try await read(transport)
        #expect(files.files.count == 103)
        #expect(!files.truncated)
        #expect(transport.requests.map { $0.url?.absoluteString } == [
            "https://api.github.com/repos/acme/app/pulls/42/files?per_page=100&page=1",
            "https://api.github.com/repos/acme/app/pulls/42/files?per_page=100&page=2",
        ])
        #expect(transport.requests.first?.value(forHTTPHeaderField: "Authorization") == "Bearer ghu_secret")
    }

    @Test func readingStopsAtThePageLimit() async throws {
        let transport = FakeTransport(try (0..<5).map { _ in try Self.page(100) })
        let files = try await read(transport)
        #expect(transport.requests.count == PullRequestFiles.pageLimit)
        #expect(files.files.count == 300)
        #expect(files.truncated)
    }

    @Test func anUnchangedPageIsReadFromItsETag() async throws {
        var first = try Self.page(2)
        first.headers["etag"] = "\"v1\""
        let transport = FakeTransport([first, GitHubResponse(status: 304)])
        let api = GitHubAPI(transport: transport.transport, now: { GitHubFixtures.start })
        let once = try await api.pullRequestFiles(host: host, owner: "acme", repo: "app", number: 42, token: token)
        let again = try await api.pullRequestFiles(host: host, owner: "acme", repo: "app", number: 42, token: token)
        #expect(once == again)
        #expect(transport.requests.last?.value(forHTTPHeaderField: "If-None-Match") == "\"v1\"")
    }

    @Test func aPatchBecomesNumberedDiffLines() {
        let lines = PullRequestFiles.lines("@@ -10,3 +10,4 @@ func a()\n keep\n-old\n+new\n+more\n\\ No newline at end of file")
        #expect(lines == [
            DiffLine(kind: .hunk, text: "@@ -10,3 +10,4 @@ func a()"),
            DiffLine(kind: .context, old: 10, new: 10, text: "keep"),
            DiffLine(kind: .removed, old: 11, text: "old"),
            DiffLine(kind: .added, new: 11, text: "new"),
            DiffLine(kind: .added, new: 12, text: "more"),
            DiffLine(kind: .meta, text: "\\ No newline at end of file"),
        ])
    }

    @Test func remoteControlsAreDroppedButTabsStay() {
        let lines = PullRequestFiles.lines("@@ -1 +1 @@\n+\tx\u{1b}[31m\u{202e}y\r")
        #expect(lines.last?.text == "\tx[31my")
    }

    @Test func aPatchPastItsCapOrTheBudgetIsNotShown() throws {
        let big = "@@ -1 +1 @@\n+" + String(repeating: "x", count: PullRequestFiles.patchBytes)
        var budget = PullRequestFiles.patchBudget
        let files = try PullRequestFiles.parse(page: try Self.page(1, patch: big).body, budget: &budget)
        #expect(files.first?.diff.shown == .tooLarge)
        #expect(files.first?.diff.lines.isEmpty == true)
        #expect(budget == PullRequestFiles.patchBudget)

        var spent = 10
        let over = try PullRequestFiles.parse(page: try Self.page(2).body, budget: &spent)
        #expect(over.map(\.diff.shown) == [.tooLarge, .tooLarge])
        #expect(spent == 10)
    }

    @Test func filesWithoutAPatchSayWhy() throws {
        let entries: [[String: Any]] = [
            ["filename": "logo.png", "status": "added", "additions": 0, "deletions": 0, "changes": 0],
            ["filename": "huge.json", "status": "modified", "additions": 9000, "deletions": 1, "changes": 9001],
            ["filename": "new.swift", "previous_filename": "old.swift", "status": "renamed", "additions": 0, "deletions": 0, "changes": 0],
            ["filename": "gone.swift", "status": "removed", "additions": 0, "deletions": 3, "changes": 3, "patch": "@@ -1,3 +0,0 @@\n-a\n-b\n-c"],
            ["filename": "", "status": "modified"],
        ]
        var budget = PullRequestFiles.patchBudget
        let files = try PullRequestFiles.parse(page: try JSONSerialization.data(withJSONObject: entries), budget: &budget)
        #expect(files.map(\.file.path) == ["logo.png", "huge.json", "new.swift", "gone.swift"])
        #expect(files.map(\.diff.shown) == [.binary, .tooLarge, .text, .text])
        #expect(files.map(\.file.status) == [.added, .modified, .renamed, .deleted])
        #expect(files[2].file.oldPath == "old.swift")
        #expect(files[3].diff.lines.count == 4)
        #expect(files[1].file.added == 9000)
    }

    @Test func aPageThatIsNotAListIsMalformed() async throws {
        let transport = FakeTransport([GitHubResponse(status: 200, body: Data("{\"message\":\"x\"}".utf8))])
        await #expect(throws: GitHubError.malformed) { try await read(transport) }
    }
}
