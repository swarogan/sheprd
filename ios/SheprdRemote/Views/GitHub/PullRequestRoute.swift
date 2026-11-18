import SheprdGitHub
import SheprdRemoteCore
import SwiftUI

/// A pull request a list opens, as a navigation value: where it lives, its
/// number, and its page on GitHub.
struct PullRequestRoute: Hashable {
    let host: GitHubHost
    let owner: String
    let repo: String
    let number: UInt64
    /// Its page, always `https` on its own host.
    let url: URL?

    init(host: GitHubHost, owner: String, repo: String, number: UInt64, url: URL?) {
        self.host = host
        self.owner = owner
        self.repo = repo
        self.number = number
        self.url = url
    }

    init(_ pull: GitHubSearchPull) {
        self.init(host: pull.host, owner: pull.owner, repo: pull.repo, number: pull.number, url: pull.url)
    }
}

extension PullRequestDetailScreen {
    /// The detail screen a list row's route leads to.
    init(route: PullRequestRoute, title: String?) {
        self.init(
            target: PullRequestTarget(host: route.host, owner: route.owner, repo: route.repo, number: route.number),
            summary: PullRequestSummary(number: route.number, title: title, url: route.url?.absoluteString)
        )
    }
}
