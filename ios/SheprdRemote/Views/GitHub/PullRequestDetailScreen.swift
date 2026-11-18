import SheprdGitHub
import SheprdRemoteCore
import SwiftUI
import UIKit

/// A pull request, read-only: its title and reviewers as chips, the
/// conversation as GitHub's timeline reads, review comments with their
/// file and line, and the files it changes, each opening its patch.
/// Everything comes straight from GitHub; with no token that reads it,
/// the screen shows what the Changes screen knew and GitHub's page.
struct PullRequestDetailScreen: View {
    @Environment(AppModel.self) private var model
    @Environment(\.scenePhase) private var phase
    /// Where it lives; nil when the workspace names no GitHub repository.
    let target: PullRequestTarget?
    let summary: PullRequestSummary
    /// What GitHub answered, stamped with `GitHubDirect.signOuts` when it
    /// was asked: an answer read before the phone signed out may have used
    /// that account, and is not shown.
    @State private var held: (reading: PullRequestReading, signOuts: Int)?
    @State private var tried = false
    /// A link from GitHub's text, waiting for the reader to confirm it.
    @State private var pendingLink: URL?

    /// How often the screen reads again while it stays in front.
    static let refresh: Duration = .seconds(60)

    private var reading: PullRequestReading? {
        guard let held, held.signOuts == GitHubDirect.shared.signOuts else { return nil }
        return held.reading
    }

    private var page: URL? {
        reading.flatMap { PullRequestLink.https($0.detail.url) } ?? summary.page
    }

    var body: some View {
        ScrollViewReader { proxy in list(proxy) }
    }

    private func list(_ proxy: ScrollViewProxy) -> some View {
        List {
            Section { titleBlock }
            if let detail = reading?.detail {
                PullRequestConversation(detail: detail)
                filesSection(detail)
            } else if tried {
                unavailable
            } else {
                Section { ProgressView().frame(maxWidth: .infinity) }
            }
        }
        .listSectionSpacing(14)
        #if DEBUG
        // `-githubDemo detail-end` shows the end, for the simulator loop.
        .onChange(of: reading?.files?.files.last?.id) { _, last in
            if let last, PullRequestDemo.anchor != nil { proxy.scrollTo(last, anchor: .bottom) }
        }
        #endif
        .navigationTitle(Text(verbatim: "#\(summary.number)"))
        .navigationBarTitleDisplayMode(.inline)
        .toolbar(.hidden, for: .tabBar)
        .toolbar {
            if let page {
                ToolbarItem(placement: .topBarTrailing) {
                    Button("Open on GitHub", systemImage: "safari") { UIApplication.shared.open(page) }
                }
            }
        }
        .refreshable { await load() }
        .onAppear { GitHubDirect.shared.watchSignIn() }
        // Reads on appearing and every minute while in front; leaving the
        // screen or the app cancels the loop, so nothing reads in the
        // background. Signing out reads again at once, with whatever token
        // is left.
        .task(id: Refresh(active: phase == .active, signOuts: GitHubDirect.shared.signOuts)) {
            if let held, held.signOuts != GitHubDirect.shared.signOuts {
                self.held = nil
                tried = false
            }
            guard phase == .active else { return }
            while !Task.isCancelled {
                await load()
                try? await Task.sleep(for: Self.refresh)
            }
        }
        // GitHub's text is untrusted: only `https` links, and only after
        // asking.
        .environment(\.openURL, OpenURLAction { url in
            guard url.scheme?.lowercased() == "https" else { return .discarded }
            pendingLink = url
            return .handled
        })
        .confirmationDialog(
            "Open this link?",
            isPresented: Binding(get: { pendingLink != nil }, set: { if !$0 { pendingLink = nil } }),
            titleVisibility: .visible,
            presenting: pendingLink
        ) { url in
            Button("Open in Safari") { UIApplication.shared.open(url) }
        } message: { url in
            Text(url.absoluteString)
        }
    }

    private struct Refresh: Equatable {
        var active: Bool
        var signOuts: Int
    }

    private func load() async {
        let signOuts = GitHubDirect.shared.signOuts
        guard let target else {
            tried = true
            return
        }
        #if DEBUG
        if let demo = await PullRequestDemo.reading(target) {
            held = (demo, signOuts)
            tried = true
            return
        }
        #endif
        let read = await GitHubReads.reader.detail(target, source: model.fleet.gitHubLenders)
        // An answer begun before a sign-out may have used that account.
        guard !Task.isCancelled, signOuts == GitHubDirect.shared.signOuts else { return }
        // A failed refresh keeps what was read before, unless that was read
        // before a sign-out.
        if let read {
            held = (read, signOuts)
        } else if let held, held.signOuts != signOuts {
            self.held = nil
        }
        tried = true
    }

    private var titleBlock: some View {
        let detail = reading?.detail
        let look = detail.map { PullRequestLook($0.state, draft: $0.isDraft) } ?? summary.look
        let title = detail?.title ?? summary.title
        let author = detail?.author ?? summary.author
        return VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 8) {
                PullRequestStatePill(look: look)
                if let repository = detail?.repository ?? target.map({ "\($0.owner)/\($0.repo)" }) {
                    Text(repository).font(.caption.monospaced()).foregroundStyle(.secondary).lineLimit(1)
                }
            }
            if let title, !title.isEmpty {
                Text(title).font(.title3.weight(.semibold)).fixedSize(horizontal: false, vertical: true)
            }
            if let author {
                HStack(spacing: 6) {
                    PullRequestAvatar(login: author, size: 18)
                    Text(byline(author: author, detail: detail)).font(.caption).foregroundStyle(.secondary)
                        .lineLimit(1).truncationMode(.middle)
                }
            }
            if let reviewers = detail?.reviewers, !reviewers.isEmpty {
                Divider().padding(.vertical, 2)
                ReviewerChips(reviewers: reviewers)
            }
        }
        .padding(.vertical, 4)
    }

    private func byline(author: String, detail: PullRequestDetail?) -> String {
        guard let detail, !detail.headRef.isEmpty, !detail.baseRef.isEmpty else { return author }
        return "\(author) · \(detail.headRef) → \(detail.baseRef)"
    }

    @ViewBuilder private var unavailable: some View {
        Section {
            ContentUnavailableView {
                Label("Not read from GitHub", systemImage: "lock")
            } description: {
                Text(target == nil
                    ? "This workspace's computer does not name its GitHub repository."
                    : "Sign in to GitHub on this phone, or share a computer's GitHub sign-in with it, to read the conversation here.")
            } actions: {
                if let page {
                    Button("Open on GitHub", systemImage: "safari") { UIApplication.shared.open(page) }
                        .buttonStyle(.glassProminent)
                }
            }
        }
    }

    @ViewBuilder private func filesSection(_ detail: PullRequestDetail) -> some View {
        if let files = reading?.files {
            Section {
                ForEach(files.files) { file in
                    NavigationLink {
                        PatchScreen(file: file)
                    } label: {
                        FileRow(file: file.file)
                    }
                }
                let unread = max(detail.changedFiles - files.files.count, 0)
                if files.truncated || unread > 0 {
                    Text(unread > 0 ? "\(unread) more files are on GitHub." : "More files are on GitHub.")
                        .font(.footnote).foregroundStyle(.secondary)
                }
            } header: {
                HStack {
                    Text(detail.changedFiles == 1 ? "1 file changed" : "\(detail.changedFiles) files changed")
                    Spacer()
                    Text(verbatim: "+\(detail.additions) −\(detail.deletions)").font(.caption.monospaced()).textCase(nil)
                }
            }
        } else {
            Section("Files changed") {
                Text("GitHub did not list the files. Pull to try again.").foregroundStyle(.secondary)
            }
        }
    }
}

/// Reviewers as a row of chips: initial, name, and their verdict.
private struct ReviewerChips: View {
    let reviewers: [PullRequestDetail.Reviewer]

    var body: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 6) {
                ForEach(reviewers) { reviewer in
                    HStack(spacing: 5) {
                        PullRequestAvatar(login: reviewer.name, team: reviewer.isTeam, size: 18)
                        Text(reviewer.name).font(.caption.weight(.medium)).lineLimit(1)
                        if let verdict = reviewer.verdict {
                            Image(systemName: verdict.symbol).font(.caption2).foregroundStyle(verdict.color)
                        }
                    }
                    .padding(.leading, 3).padding(.trailing, 8).padding(.vertical, 3)
                    .background(.fill.tertiary, in: .capsule)
                    .accessibilityElement(children: .ignore)
                    .accessibilityLabel(label(reviewer))
                }
            }
        }
    }

    private func label(_ reviewer: PullRequestDetail.Reviewer) -> String {
        switch reviewer.verdict {
        case .approved?: "\(reviewer.name) approved"
        case .changesRequested?: "\(reviewer.name) requested changes"
        case .commented?: "\(reviewer.name) commented"
        case .dismissed?: "\(reviewer.name)'s review was dismissed"
        case .pending?, nil: "\(reviewer.name), review requested"
        }
    }
}

/// One file's patch as GitHub sent it, in the Changes screen's diff view.
private struct PatchScreen: View {
    let file: PullRequestFile

    var body: some View {
        Group {
            if file.diff.lines.isEmpty {
                ContentUnavailableView("Nothing to show", systemImage: "doc.text", description: Text(file.diff.note ?? ""))
            } else {
                DiffView(diff: file.diff)
            }
        }
        .navigationTitle((file.file.path as NSString).lastPathComponent)
        .navigationBarTitleDisplayMode(.inline)
    }
}
