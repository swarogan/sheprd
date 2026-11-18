import SheprdGitHub
import SheprdRemoteCore
import SwiftUI

/// The pull requests the github.com account wrote, and those asking for its
/// review, on any repository, including ones no computer has a workspace
/// for. Each says where its branch is checked out. Review requests come
/// first while there are any: they wait on the person.
///
/// The Pull Requests tab, beside Workspaces: it reads the lists while it is
/// in front, as the Workspaces tab reads its badges.
struct PullRequestsList: View {
    @Environment(AppModel.self) private var model
    /// A workspace opened from a row's menu.
    @State private var opened: WorkspaceKey?
    /// Words a pull request's title, repository, number, author, or branch
    /// must hold to be listed.
    @State private var query = ""
    @AppStorage(PullRequestSort.key) private var sort = PullRequestSort.updated

    /// Where each section's header is, so a jump from the edge index can
    /// clear the search field, as on Home.
    @State private var headerJump = HeaderJump()

    private var direct: GitHubDirect { GitHubDirect.shared }

    /// Pull requests listed before the edge index shows: about a screenful.
    private static let indexedFrom = 12

    /// The two sections, by the id their header and rows scroll by: a
    /// list scrolls to a row by its `ForEach` id, so the rows are listed
    /// by these, as a pull request may be in both sections.
    private enum Part: String {
        case requested, mine

        var header: String { "pulls/\(rawValue)" }
        func row(_ pull: GitHubSearchPull) -> String { "pulls/\(rawValue)/\(pull.id)" }
    }

    /// A pull request as one section lists it, identified as the edge
    /// index scrolls to it.
    private struct Listed: Identifiable {
        var part: Part
        var pull: GitHubSearchPull
        var id: String { part.row(pull) }
    }

    /// The lists as shown: searched, then sorted.
    private var shown: PullRequestLists? {
        direct.lists.lists?.matching(query).sorted(sort)
    }

    var body: some View {
        ScrollViewReader { proxy in
            list
                .overlay(alignment: .trailing) {
                    if let shown, shown.mine.count + shown.reviewRequested.count >= Self.indexedFrom {
                        EdgeIndexStrip(parts: indexParts(shown)) { jump(to: $0, in: shown, proxy: proxy) }
                            .padding(.vertical, 24)
                            .padding(.trailing, 2)
                    }
                }
        }
        .searchable(text: $query, placement: .navigationBarDrawer(displayMode: .always), prompt: "Title, repository, author, branch")
        // No title, as Home and Workspaces have none; the search heads it.
        .navigationBarTitleDisplayMode(.inline)
        #if DEBUG
        .task { if let text = UserDefaults.standard.string(forKey: "pullRequestSearch") { query = text } }
        #endif
        .toolbar {
            ToolbarItem(placement: .topBarLeading) { PullRequestSortMenu(sort: $sort) }
        }
        .gitHubRefresh(showsLists: true)
        .navigationDestination(for: PullRequestRoute.self) { route in
            PullRequestDetailScreen(route: route, title: direct.title(for: route))
        }
        .navigationDestination(item: $opened) { WorkspaceScreen(key: $0) }
    }

    private var list: some View {
        List {
            if let lists = shown {
                if case .failed = direct.lists {
                    Section {
                        Label("GitHub could not be read. These may be out of date.", systemImage: "exclamationmark.triangle")
                            .font(.footnote).foregroundStyle(.secondary)
                    }
                }
                if lists.reviewRequested.isEmpty {
                    mine(lists)
                } else {
                    requested(lists)
                    mine(lists)
                }
            }
        }
        .listStyle(.insetGrouped)
        .listSectionSpacing(14)
        .refreshable { await direct.refreshLists(source: model.fleet.gitHubLenders) }
        .overlay { empty }
    }

    /// The sections as the edge index reads them: Review requested, when
    /// any ask, then Mine.
    private func indexParts(_ lists: PullRequestLists) -> [EdgeIndex.Part] {
        let parts: [(Part, String, [GitHubSearchPull])] = [
            (.requested, "Review requested", lists.reviewRequested), (.mine, "Mine", lists.mine),
        ]
        return parts.filter { !$0.2.isEmpty }.map { part, title, pulls in
            EdgeIndex.Part(target: part.header, title: title, rows: pulls.map { (part.row($0), $0.dot) })
        }
    }

    /// A section's first row shows its header too, as on Home.
    private func jump(to target: String, in lists: PullRequestLists, proxy: ScrollViewProxy) {
        let firsts = [(Part.requested, lists.reviewRequested.first), (Part.mine, lists.mine.first)]
        let header = firsts.first { part, pull in pull.map(part.row) == target }?.0.header
        headerJump.go(to: header ?? target, proxy: proxy)
    }

    @ViewBuilder private func mine(_ lists: PullRequestLists) -> some View {
        if !lists.mine.isEmpty {
            Section {
                ForEach(lists.mine.map { Listed(part: .mine, pull: $0) }) { row($0.pull, showsAuthor: false) }
            } header: {
                header("Mine", shown: lists.mine.count, total: lists.mineTotal, part: .mine)
            }
        }
    }

    @ViewBuilder private func requested(_ lists: PullRequestLists) -> some View {
        Section {
            ForEach(lists.reviewRequested.map { Listed(part: .requested, pull: $0) }) { row($0.pull, showsAuthor: true) }
        } header: {
            header("Review requested", shown: lists.reviewRequested.count, total: lists.reviewRequestedTotal, part: .requested)
        }
    }

    private func header(_ title: String, shown: Int, total: Int, part: Part) -> some View {
        HStack {
            Text(title)
            Spacer()
            if total > shown { Text("\(shown) of \(total)").textCase(nil) }
        }
        // A read the person waits on shows the list's own pull-to-refresh
        // spinner, as if they had pulled.
        .background(RefreshSpinner(active: direct.loadingLists))
        .id(part.header)
        .background(HeaderMark(jump: headerJump, key: part.header))
    }

    private func row(_ pull: GitHubSearchPull, showsAuthor: Bool) -> some View {
        let places = ComputerWorkspaces.places(of: pull, in: computers)
        return NavigationLink(value: PullRequestRoute(pull)) {
            PullRequestListRow(pull: pull, showsAuthor: showsAuthor, places: places)
        }
        .contextMenu {
            ForEach(places, id: \.self) { place in
                Button("Open Workspace on \(place.computer)", systemImage: "laptopcomputer") {
                    opened = WorkspaceKey(device: place.device, workspace: place.workspace)
                }
            }
            if let url = pull.url {
                Link(destination: url) { Label("Open on GitHub", systemImage: "safari") }
                ShareLink(item: url)
            }
        }
    }

    private var computers: [ComputerWorkspaces] {
        #if DEBUG
        if GitHubSample.enabled { return GitHubSample.computers }
        #endif
        return model.fleet.computerWorkspaces
    }

    @ViewBuilder private var empty: some View {
        if let lists = direct.lists.lists, !lists.isEmpty, lists.matching(query).isEmpty {
            ContentUnavailableView.search(text: query)
        } else if direct.loadingLists, direct.lists.lists == nil {
            // Rather than what an earlier read said, while a new one runs.
            LoadingPullRequests()
        } else {
            unloaded
        }
    }

    @ViewBuilder private var unloaded: some View {
        switch direct.lists {
        case .noAccount:
            ContentUnavailableView {
                Label("No GitHub account", systemImage: "arrow.triangle.pull")
            } description: {
                Text("Sign in to GitHub on this \(UIDevice.current.model), or let a paired computer lend its sign-in.")
            } actions: {
                NavigationLink("Open Settings › GitHub") { GitHubSettingsView() }
                    .buttonStyle(.borderedProminent)
            }
        case .unread:
            LoadingPullRequests()
        case let .read(lists) where lists.isEmpty:
            ContentUnavailableView("No open pull requests", systemImage: "arrow.triangle.pull",
                                   description: Text("Pull requests you opened, and those asking for your review, appear here."))
        case .failed(nil):
            ContentUnavailableView("GitHub could not be read", systemImage: "exclamationmark.triangle",
                                   description: Text("Pull down to try again."))
        case .read, .failed:
            EmptyView()
        }
    }
}

/// The order of the Pull Requests tab, at its top left as Home's is.
private struct PullRequestSortMenu: View {
    @Binding var sort: PullRequestSort

    var body: some View {
        Menu {
            Picker("Sort Pull Requests", selection: $sort.animation()) {
                ForEach(PullRequestSort.allCases) { sort in
                    switch sort {
                    case .updated: Label("Recently Updated", systemImage: "clock").tag(sort)
                    case .status: Label("Status", systemImage: "exclamationmark.circle").tag(sort)
                    }
                }
            }
        } label: {
            Image(systemName: "arrow.up.arrow.down")
        }
        .accessibilityLabel("Sort pull requests")
    }
}

/// A spinner saying the lists are being read from GitHub.
private struct LoadingPullRequests: View {
    var body: some View {
        HStack(spacing: 8) {
            ProgressView()
            Text("Loading pull requests…").foregroundStyle(.secondary)
        }
        .frame(maxWidth: .infinity)
    }
}

/// A pull request's row: repository and number, title, review or author,
/// checks, and where its branch is checked out.
private struct PullRequestListRow: View {
    var pull: GitHubSearchPull
    var showsAuthor: Bool
    var places: [WorkspacePlace]

    var body: some View {
        let look = PullRequestLook(pull.state, draft: pull.isDraft)
        VStack(alignment: .leading, spacing: 5) {
            HStack(spacing: 6) {
                Image(systemName: look.symbol).font(.caption.weight(.semibold)).foregroundStyle(look.color)
                Text(verbatim: "\(pull.repository) #\(pull.number)").font(.caption.monospaced())
                    .foregroundStyle(.secondary).lineLimit(1).truncationMode(.middle)
                Spacer(minLength: 4)
                Text(verbatim: PullRequestLink.age(pull.updatedAt)).font(.caption).foregroundStyle(.secondary)
            }
            Text(verbatim: pull.title).lineLimit(2)
            HStack(spacing: 8) {
                if showsAuthor, let author = pull.author {
                    PullRequestAvatar(login: author, size: 16)
                    Text(verbatim: author).font(.caption).foregroundStyle(.secondary).lineLimit(1)
                } else if let review = pull.reviewDecision.look {
                    IconText(review.label, systemImage: review.symbol)
                        .font(.caption.weight(.medium)).foregroundStyle(review.color).lineLimit(1)
                } else {
                    Text(look.label).font(.caption).foregroundStyle(.secondary)
                }
                Spacer(minLength: 4)
                CheckCountsView(counts: pull.checks)
            }
            whereLine
        }
        .padding(.vertical, 2)
        .accessibilityElement(children: .combine)
    }

    private var whereLine: some View {
        Group {
            if let first = places.first {
                let others = Set(places.map(\.device)).subtracting([first.device]).count
                IconText(others == 0 ? "Workspace on \(first.computer)" : "Workspace on \(first.computer) and \(others) more",
                         systemImage: "laptopcomputer")
            } else {
                IconText("No workspace on any computer", systemImage: "icloud")
            }
        }
        .font(.caption2)
        .foregroundStyle(places.isEmpty ? AnyShapeStyle(.tertiary) : AnyShapeStyle(.secondary))
        .lineLimit(1)
    }
}

/// Failing, pending, and passed checks as small colored numbers.
private struct CheckCountsView: View {
    var counts: GitHubSearchPull.CheckCounts

    var body: some View {
        HStack(spacing: 8) {
            if counts.failing > 0 { count(counts.failing, "xmark.circle.fill", .red) }
            if counts.pending > 0 { count(counts.pending, "clock.fill", .yellow) }
            if counts.passed > 0 { count(counts.passed, "checkmark.circle.fill", .green) }
        }
        .font(.caption.weight(.semibold).monospacedDigit())
        .fixedSize()
        .accessibilityElement(children: .ignore)
        .accessibilityLabel("\(counts.failing) failing, \(counts.pending) pending, \(counts.passed) passed checks")
    }

    private func count(_ value: Int, _ symbol: String, _ color: Color) -> some View {
        HStack(spacing: 3) {
            Image(systemName: symbol).foregroundStyle(color)
            Text(verbatim: "\(value)").foregroundStyle(.secondary)
        }
    }
}

/// An icon and its text side by side. A `Label` in a list row takes the
/// row's icon column, which spreads these small captions apart.
struct IconText: View {
    var title: String
    var systemImage: String

    init(_ title: String, systemImage: String) {
        self.title = title
        self.systemImage = systemImage
    }

    var body: some View {
        HStack(spacing: 4) {
            Image(systemName: systemImage)
            Text(verbatim: title)
        }
    }
}

private extension GitHubSearchPull {
    /// Its dot on the edge index, in the agents' palette: red when it needs
    /// the person (changes asked for, a check failing), yellow while checks
    /// run, teal once approved, green otherwise.
    var dot: Home.AgentStatus {
        if reviewDecision == .changesRequested || checks.failing > 0 { return .blocked }
        if checks.pending > 0 { return .working }
        if reviewDecision == .approved { return .done }
        return .idle
    }
}
