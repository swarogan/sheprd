#if DEBUG
import Foundation
import SheprdGitHub
import SheprdRemoteCore

/// Made-up GitHub answers for screenshots with no computer or account:
/// `-gitHubDemo YES` fills the pull request lists and Settings > GitHub
/// (`-gitHubDemo signedIn` for the signed-in page), and `-gitHubSignIn
/// STEP` opens the sign-in sheet on a step.
enum GitHubSample {
    static var enabled: Bool { UserDefaults.standard.string(forKey: "gitHubDemo") != nil }
    private static var signedIn: Bool { UserDefaults.standard.string(forKey: "gitHubDemo") == "signedIn" }

    private static let host = GitHubHost("github.com")!
    private static let enterprise = GitHubHost("github.acme-corp.com")!

    private static func pull(
        _ owner: String, _ repo: String, _ number: UInt64, _ title: String, ago: TimeInterval,
        draft: Bool = false, review: GitHubPullRequest.ReviewDecision = .none, author: String = "octocat",
        branch: String, checks: GitHubSearchPull.CheckCounts
    ) -> GitHubSearchPull {
        GitHubSearchPull(
            host: host, owner: owner, repo: repo, number: number, title: title, isDraft: draft, reviewDecision: review,
            author: author, updatedAt: .now.addingTimeInterval(-ago), headRefName: branch, headOwner: owner, checks: checks
        )
    }

    static var lists: PullRequestLists {
        PullRequestLists(
            mine: [
                pull("sheprd-labs", "sheprd", 1342, "Read pull requests on the phone with a lent GitHub sign-in", ago: 2400,
                     review: .changesRequested, branch: "feat/ios-native-github", checks: .init(failing: 2, pending: 1, passed: 5)),
                pull("sheprd-labs", "sheprd", 1338, "Keep the relay's dial keys after a restart", ago: 3 * 3600,
                     review: .approved, branch: "fix/relay-dial-keys", checks: .init(passed: 14)),
                pull("sheprd-labs", "herdr-site", 214, "Docs: GitHub on the phone", ago: 86400, draft: true,
                     branch: "docs/phone", checks: .init(pending: 2)),
                pull("sheprd-labs", "sheprd", 1301, "Group tabs by worktree in the iPad sidebar", ago: 2 * 86400,
                     review: .approved, branch: "feat/ipad-groups", checks: .init(passed: 14)),
            ],
            reviewRequested: [
                pull("sheprd-labs", "sheprd", 1345, "Teleport: carry checkpoints across hosts", ago: 720, author: "hubot",
                     branch: "feat/teleport-checkpoints", checks: .init(pending: 3, passed: 9)),
                pull("sheprd-labs", "sheprd-apns", 57, "Rotate the provider token before it expires", ago: 2 * 3600, author: "monalisa",
                     branch: "rotate-token", checks: .init(failing: 1, passed: 6)),
                pull("octo-org", "infra", 902, "Bump relay droplets to 2 vCPU", ago: 5 * 3600, author: "dependabot",
                     branch: "dependabot/droplets", checks: .init(passed: 4)),
            ]
        )
    }

    /// Workspaces carrying some of the sample branches.
    static var computers: [ComputerWorkspaces] {
        guard enabled else { return [] }
        func workspace(_ repo: String, _ branch: String) -> Home.Workspace? {
            let json = #"{"id":"\#(branch)","label":"\#(branch)","cwd":"/","chats":[],"terminals":[],"github":{"host":"github.com","owner":"sheprd-labs","repo":"\#(repo)","branch":"\#(branch)"}}"#
            return try? JSONDecoder().decode(Home.Workspace.self, from: Data(json.utf8))
        }
        return [
            ComputerWorkspaces(device: "sample-mbp", computer: "MacBook Pro", workspaces: [workspace("sheprd", "feat/ios-native-github")].compactMap { $0 }),
            ComputerWorkspaces(device: "sample-studio", computer: "studio", workspaces: [workspace("sheprd", "fix/relay-dial-keys")].compactMap { $0 }),
            ComputerWorkspaces(device: "sample-build", computer: "build-01", workspaces: [workspace("sheprd", "feat/teleport-checkpoints")].compactMap { $0 }),
        ]
    }

    static func settings() -> [GitHubHostAccount]? {
        guard enabled else { return nil }
        let lent = GitHubHostAccount.Lent(login: "octocat", computer: "MacBook Pro", source: .sheprd, expiresAt: .now.addingTimeInterval(6 * 3600 + 60))
        let github = GitHubHostAccount(
            host: host,
            own: signedIn ? .init(login: "octocat", expiresAt: .now.addingTimeInterval(7 * 3600 + 60), access: .installed) : nil,
            lent: lent
        )
        let acme = GitHubHostAccount(host: enterprise, lent: .init(login: "octocat-acme", computer: "studio", source: .gh, expiresAt: nil))
        return [github, acme]
    }

    static let organizations = [
        GitHubOrganization(login: "sheprd-labs", access: .all),
        GitHubOrganization(login: "octo-org", access: .publicOnly),
    ]
}
#endif
