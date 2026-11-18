import SheprdGitHub
import SheprdRemoteCore
import SwiftUI

/// What the Workspaces tab reads from GitHub itself: workspace badges by
/// branch, and the pull request lists. Both refresh while the tab is in
/// front, and only then; signing out of the phone's own account drops
/// them, as they may have been read with it.
@MainActor
@Observable
final class GitHubDirect {
    static let shared = GitHubDirect()

    /// Starts with the lists last read, from the cache, off the main
    /// thread; a read that answered first wins.
    private init() {
        Task {
            let cached = await Task.detached(priority: .userInitiated) { Cache.shared.pullRequestLists() }.value
            guard let cached, case .unread = lists else { return }
            lists = .read(cached)
        }
    }

    /// The pull request lists, as last read.
    enum Lists: Equatable {
        case unread
        case noAccount
        /// GitHub could not be read; `last` stays shown when there was one.
        case failed(last: PullRequestLists?)
        case read(PullRequestLists)

        var lists: PullRequestLists? {
            switch self {
            case let .read(lists): lists
            case let .failed(last): last
            case .unread, .noAccount: nil
            }
        }
    }

    private(set) var badges = DirectBadges()
    private(set) var lists = Lists.unread
    /// Whether a read the person is waiting on runs: the first since the
    /// lists appeared, not the quiet one each minute after it.
    private(set) var loadingLists = false
    /// The phone's own github.com login, while signed in.
    private(set) var ownLogin: String?
    @ObservationIgnored private var watching: Task<Void, Never>?
    @ObservationIgnored private var readingBadges = false
    @ObservationIgnored private var readingLists = false
    /// Bumped when signing out, so reads begun before it keep nothing.
    /// Screens holding their own GitHub answers stamp them with it, and
    /// drop and read again those stamped before a sign-out.
    private(set) var signOuts = 0

    /// How often what is on screen is read again.
    static let refresh: Duration = .seconds(60)

    private var reader: PullRequestReader { GitHubReads.reader }

    /// The badge `workspace`'s row shows.
    func badge(for workspace: Home.Workspace) -> Home.PullRequest? {
        badges.badge(for: workspace).pullRequest
    }

    /// The title a listed pull request had, for a screen opened from it.
    func title(for route: PullRequestRoute) -> String? {
        let all = (lists.lists?.mine ?? []) + (lists.lists?.reviewRequested ?? [])
        return all.first { PullRequestRoute($0) == route }?.title
    }

    /// Follows the phone's own sign-in, once, to drop what was read with
    /// it when it signs out.
    func watchSignIn() {
        guard watching == nil else { return }
        watching = Task { [weak self] in
            guard let signIn = await GitHubReads.reader.signIn else { return }
            for await state in await signIn.states() {
                self?.signInChanged(state)
            }
        }
    }

    private func signInChanged(_ state: GitHubSignInState) {
        switch state {
        case let .signedIn(login, _):
            ownLogin = login
        case .idle, .failed(.refreshRejected), .failed(.signedOut):
            guard ownLogin != nil else { return }
            ownLogin = nil
            forgetDirectReads()
        case .requesting, .waiting, .failed:
            // A sign-in held stays in use while another is under way.
            break
        }
    }

    /// Drops every answer read from GitHub; the next refresh reads again
    /// with whatever token is left.
    func forgetDirectReads() {
        signOuts += 1
        badges = DirectBadges()
        lists = .unread
        Cache.shared.save(pullRequestLists: nil)
        Task { await reader.forgetAnswers() }
    }

    /// Reads the badges of `workspaces` from GitHub.
    func refreshBadges(_ workspaces: [Home.Workspace], source: Lenders) async {
        guard !readingBadges else { return }
        readingBadges = true
        defer { readingBadges = false }
        let mine = signOuts
        let (asked, branches) = DirectBadges.requests(workspaces)
        guard !asked.isEmpty else {
            badges = DirectBadges()
            return
        }
        let found = await reader.pullRequests(workspaces: asked, source: source)
        guard mine == signOuts, !Task.isCancelled else { return }
        let next = DirectBadges.answered(found, branches: branches)
        if next != badges { badges = next }
    }

    /// Reads the pull request lists; `shown` when the person is waiting on
    /// it, so the lists say they are loading.
    func refreshLists(source: Lenders, shown: Bool = false) async {
        guard !readingLists else { return }
        readingLists = true
        if shown { loadingLists = true }
        defer {
            readingLists = false
            loadingLists = false
        }
        let mine = signOuts
        let read = await reader.pullRequestLists(source: source)
        guard mine == signOuts, !Task.isCancelled else { return }
        switch read {
        case let .found(found):
            lists = .read(found)
            Cache.shared.save(pullRequestLists: found)
        case .noAccount:
            lists = .noAccount
            Cache.shared.save(pullRequestLists: nil)
        case .failed: lists = .failed(last: lists.lists)
        }
    }

    #if DEBUG
    /// Sample answers for screenshots, with no computer or account.
    func showSample(_ lists: PullRequestLists) {
        self.lists = .read(lists)
    }

    /// Holds the lists as loading for `seconds`, as a read would.
    func showSampleLoading(for seconds: Int) async {
        loadingLists = true
        try? await Task.sleep(for: .seconds(seconds))
        loadingLists = false
    }
    #endif
}

extension Fleet {
    /// The workspaces whose badges the Workspaces tab shows: on computers
    /// not collapsed.
    func visibleWorkspaces(collapsed: (String) -> Bool) -> [Home.Workspace] {
        machines.filter { !collapsed($0.id) }.flatMap { $0.home?.workspaces ?? [] }
    }

    /// Every computer's workspaces, for finding where a pull request is
    /// checked out.
    var computerWorkspaces: [ComputerWorkspaces] {
        machines.map { ComputerWorkspaces(device: $0.id, computer: $0.computer.name, workspaces: $0.home?.workspaces ?? []) }
    }
}

/// Reads what the Workspaces tab shows from GitHub when it appears, then
/// every minute while it stays in front with the app active: badges for
/// the computers open, and the lists while they are shown. Nothing runs
/// in the background.
private struct GitHubRefresh: ViewModifier {
    @Environment(AppModel.self) private var model
    @Environment(\.scenePhase) private var phase
    var showsLists: Bool

    private struct Key: Equatable {
        var active: Bool
        var lists: Bool
        var login: String?
    }

    func body(content: Content) -> some View {
        let direct = GitHubDirect.shared
        content
            .onAppear { direct.watchSignIn() }
            .task(id: Key(active: phase == .active, lists: showsLists, login: direct.ownLogin)) {
                guard phase == .active else { return }
                #if DEBUG
                if GitHubSample.enabled {
                    direct.showSample(GitHubSample.lists)
                    // `-gitHubDemoLoading N` holds the lists loading N seconds.
                    let seconds = UserDefaults.standard.integer(forKey: "gitHubDemoLoading")
                    if showsLists, seconds > 0 { await direct.showSampleLoading(for: seconds) }
                    return
                }
                #endif
                // The first read once the lists show is one the person waits on.
                var first = true
                while !Task.isCancelled {
                    let source = model.fleet.gitHubLenders
                    if showsLists {
                        await direct.refreshLists(source: source, shown: first)
                        first = false
                    } else {
                        let shown = model.fleet.visibleWorkspaces { model.collapsed.computers.contains($0) }
                        await direct.refreshBadges(shown, source: source)
                    }
                    try? await Task.sleep(for: GitHubDirect.refresh)
                }
            }
    }
}

extension View {
    /// Keeps the Workspaces tab's GitHub reads fresh while it is in front.
    func gitHubRefresh(showsLists: Bool) -> some View {
        modifier(GitHubRefresh(showsLists: showsLists))
    }
}
