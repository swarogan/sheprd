import Foundation
import SheprdRemoteCore

/// A branch's pull request as the Changes screen's header shows it: the
/// whole pull request when GitHub answered, only its checks (and their
/// number) when the computer did.
public struct BranchPullRequest: Hashable, Sendable {
    public var checks: PullRequestChecks
    /// Title, state, and review; nil when the computer answered, or when
    /// the branch has none.
    public var pullRequest: GitHubPullRequest?

    public init(checks: PullRequestChecks, pullRequest: GitHubPullRequest? = nil) {
        self.checks = checks
        self.pullRequest = pullRequest
    }
}

/// The pull request a detail screen reads: where it lives and its number.
public struct PullRequestTarget: Hashable, Sendable {
    public var host: GitHubHost
    public var owner: String
    public var repo: String
    public var number: UInt64

    public init(host: GitHubHost, owner: String, repo: String, number: UInt64) {
        self.host = host
        self.owner = owner
        self.repo = repo
        self.number = number
    }

    /// `workspace`'s pull request numbered `number`, in the repository its
    /// computer names; nil when it names none on a host the phone knows.
    public init?(workspace: Home.Workspace, number: UInt64) {
        guard let github = workspace.github, let host = GitHubHost(github.host) else { return nil }
        self.init(host: host, owner: github.owner, repo: github.repo, number: number)
    }
}

/// What a detail read found: the pull request, and its files unless that
/// second read failed.
public struct PullRequestReading: Hashable, Sendable {
    public var detail: PullRequestDetail
    public var files: PullRequestFiles?

    public init(detail: PullRequestDetail, files: PullRequestFiles?) {
        self.detail = detail
        self.files = files
    }
}

extension PullRequestReader {
    /// `workspace`'s pull request, read from GitHub with the same tokens
    /// as `checks`, else its checks from the computer.
    public func branchPullRequest(
        workspace: Home.Workspace,
        computer: String,
        source: Lenders,
        askComputer: @Sendable () async throws -> PullRequestChecks
    ) async throws -> Sourced<BranchPullRequest> {
        let found = await pullRequests(workspaces: [workspace], source: source)
        if let direct = found[workspace.id] {
            let checks = direct?.checkList ?? PullRequestChecks(checks: [])
            return Sourced(BranchPullRequest(checks: checks, pullRequest: direct), from: .github)
        }
        return Sourced(BranchPullRequest(checks: try await askComputer()), from: .computer(computer))
    }

    /// `target`'s detail and files, straight from GitHub; nil when no
    /// token reads it. The computer has no conversation to fall back on,
    /// so the screen then offers GitHub's page instead.
    public func detail(_ target: PullRequestTarget, source: Lenders) async -> PullRequestReading? {
        await attempt(host: target.host, source: source) { api, token in
            let detail = try await api.pullRequestDetail(
                host: target.host, owner: target.owner, repo: target.repo, number: target.number, token: token
            )
            // The conversation still shows when the files cannot be read,
            // but a refused token goes back so it is renewed and retried.
            let files: PullRequestFiles?
            do {
                files = try await api.pullRequestFiles(
                    host: target.host, owner: target.owner, repo: target.repo, number: target.number, token: token
                )
            } catch GitHubError.unauthorized {
                throw GitHubError.unauthorized
            } catch {
                files = nil
            }
            return PullRequestReading(detail: detail, files: files)
        }
    }
}
