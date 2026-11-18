import Foundation
import SheprdRemoteCore

/// Workspace badges read from GitHub by the phone itself, by branch, so a
/// branch checked out on two computers costs one read and shows one badge.
/// A workspace GitHub has not answered for keeps its computer's badge.
public struct DirectBadges: Equatable, Sendable {
    /// GitHub's answer per branch: its pull request, or none (nil).
    public private(set) var answers: [GitHubBranch: Home.PullRequest?] = [:]

    public init(answers: [GitHubBranch: Home.PullRequest?] = [:]) {
        self.answers = answers
    }

    /// Workspaces read at most per refresh: a screenful many times over.
    public static let limit = 200

    /// Whose badge a workspace shows.
    public enum Choice: Equatable, Sendable {
        /// GitHub's, read by the phone: a pull request, or none.
        case github(Home.PullRequest?)
        /// The computer's, from its `home`.
        case computer(Home.PullRequest?)

        public var pullRequest: Home.PullRequest? {
            switch self {
            case let .github(pullRequest), let .computer(pullRequest): pullRequest
            }
        }
    }

    /// GitHub's answer for `workspace`'s branch when there is one, else the
    /// computer's.
    public func badge(for workspace: Home.Workspace) -> Choice {
        if let branch = workspace.github.flatMap(GitHubBranch.init), let answer = answers[branch] { return .github(answer) }
        return .computer(workspace.pullRequest)
    }

    /// What to ask GitHub about `workspaces`: one workspace per branch,
    /// its id the branch's, at most `limit`, and the branch each id is.
    /// Ids repeat across computers; branches do not.
    public static func requests(_ workspaces: [Home.Workspace]) -> (workspaces: [Home.Workspace], branches: [String: GitHubBranch]) {
        var asked: [Home.Workspace] = []
        var branches: [String: GitHubBranch] = [:]
        for workspace in workspaces where asked.count < limit {
            guard let branch = workspace.github.flatMap(GitHubBranch.init) else { continue }
            let id = branch.description
            guard branches[id] == nil else { continue }
            branches[id] = branch
            var request = workspace
            request.id = id
            asked.append(request)
        }
        return (asked, branches)
    }

    /// The badges after a refresh: what GitHub answered now, by the ids
    /// `requests` gave. A branch it did not answer for goes back to its
    /// computer's badge rather than keeping an old answer.
    public static func answered(_ found: [String: GitHubPullRequest?], branches: [String: GitHubBranch]) -> DirectBadges {
        var answers: [GitHubBranch: Home.PullRequest?] = [:]
        for (id, pullRequest) in found {
            guard let branch = branches[id] else { continue }
            answers[branch] = .some(pullRequest?.badge)
        }
        return DirectBadges(answers: answers)
    }
}
