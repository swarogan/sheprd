import SwiftUI

// Sample data and small parts for the GitHub mockups (docs/ios-github-plan.md,
// phase 4). Repositories, logins and numbers are made up.

enum GHState {
    case open, draft, merged, closed

    var label: String {
        switch self {
        case .open: "Open"
        case .draft: "Draft"
        case .merged: "Merged"
        case .closed: "Closed"
        }
    }

    var symbol: String {
        switch self {
        case .open: "arrow.triangle.pull"
        case .draft: "pencil.circle"
        case .merged: "arrow.triangle.merge"
        case .closed: "xmark.circle"
        }
    }

    var color: Color {
        switch self {
        case .open: .green
        case .draft: .secondary
        case .merged: .purple
        case .closed: .red
        }
    }
}

enum GHReview {
    case approved, changesRequested, required

    var label: String {
        switch self {
        case .approved: "Approved"
        case .changesRequested: "Changes requested"
        case .required: "Review required"
        }
    }

    var symbol: String {
        switch self {
        case .approved: "checkmark.seal.fill"
        case .changesRequested: "exclamationmark.bubble.fill"
        case .required: "eye"
        }
    }

    var color: Color {
        switch self {
        case .approved: .green
        case .changesRequested: .red
        case .required: .orange
        }
    }
}

enum GHCheckState { case failed, pending, passed, skipped }

struct GHCheck: Identifiable {
    var name: String
    var state: GHCheckState
    var detail: String
    var hasLog = false
    var id: String { name }

    var symbol: String {
        switch state {
        case .passed: "checkmark.circle.fill"
        case .failed: "xmark.circle.fill"
        case .pending: "clock"
        case .skipped: "minus.circle"
        }
    }

    var color: Color {
        switch state {
        case .passed: .green
        case .failed: .red
        case .pending: .yellow
        case .skipped: .secondary
        }
    }
}

struct GHPerson: Identifiable {
    var login: String
    var tint: Color
    var id: String { login }
}

struct GHReviewer: Identifiable {
    var person: GHPerson
    var verdict: GHReview?
    var isTeam = false
    var id: String { person.login }
}

struct GHComment: Identifiable {
    let id = UUID()
    var author: GHPerson
    var when: String
    var body: String
    /// A review comment's file and line; nil for the conversation.
    var path: String? = nil
    var verdict: GHReview? = nil
    var replies: Int = 0
}

struct GHFile: Identifiable {
    var path: String
    var added: Int
    var removed: Int
    var id: String { path }
}

struct GHPull: Identifiable {
    var repo: String
    var number: Int
    var title: String
    var state: GHState
    var review: GHReview?
    var author: GHPerson
    var branch: String
    var updated: String
    var failing = 0
    var pending = 0
    var passed = 0
    /// The computer with a workspace on this branch, if any.
    var workspace: String? = nil
    var comments = 0
    var id: String { "\(repo)#\(number)" }
}

struct GHAccount: Identifiable {
    var host: String
    var login: String
    var name: String
    var tint: Color
    var lentBy: String
    var source: String
    var expires: String?
    var others: [String] = []
    var id: String { host }
}

enum GH {
    static let octocat = GHPerson(login: "octocat", tint: .indigo)
    static let hubot = GHPerson(login: "hubot", tint: .teal)
    static let monalisa = GHPerson(login: "monalisa", tint: .pink)
    static let mona = GHPerson(login: "mona-ios", tint: .orange)
    static let dependabot = GHPerson(login: "dependabot", tint: .blue)

    static let repo = "sheprd-labs/sheprd"
    static let branch = "phone-github-reads"
    static let title = "Read pull requests on the phone with a lent GitHub sign-in"

    static let checks: [GHCheck] = [
        GHCheck(name: "clippy (linux, all)", state: .failed, detail: "Failed after 4m 12s", hasLog: true),
        GHCheck(name: "test (windows, default)", state: .failed, detail: "Failed after 11m 40s", hasLog: true),
        GHCheck(name: "windows-native (arm64)", state: .pending, detail: "Running for 6m"),
        GHCheck(name: "fmt", state: .passed, detail: "18s"),
        GHCheck(name: "commits", state: .passed, detail: "9s"),
        GHCheck(name: "test (macos, all)", state: .passed, detail: "8m 02s"),
        GHCheck(name: "ios (swift test)", state: .passed, detail: "3m 51s"),
        GHCheck(name: "windows-package", state: .skipped, detail: "Skipped"),
    ]

    static var failing: [GHCheck] { checks.filter { $0.state == .failed } }
    static var pending: [GHCheck] { checks.filter { $0.state == .pending } }
    static var settled: [GHCheck] { checks.filter { $0.state == .passed || $0.state == .skipped } }

    static let files: [GHFile] = [
        GHFile(path: "ios/SheprdRemoteCore/Sources/GitHub/PullRequestQuery.swift", added: 214, removed: 0),
        GHFile(path: "ios/SheprdRemoteCore/Sources/GitHub/Lease.swift", added: 132, removed: 4),
        GHFile(path: "ios/SheprdRemote/Views/ChangesScreen.swift", added: 88, removed: 31),
        GHFile(path: "crates/sheprd-remote-control/src/server/github.rs", added: 176, removed: 12),
        GHFile(path: "docs/ios-github-plan.md", added: 9, removed: 3),
    ]

    static let reviewers: [GHReviewer] = [
        GHReviewer(person: hubot, verdict: .approved),
        GHReviewer(person: monalisa, verdict: .changesRequested),
        GHReviewer(person: GHPerson(login: "sheprd-labs/ios", tint: .gray), verdict: nil, isTeam: true),
    ]

    static let body = """
    The phone now asks GitHub itself, with a sign-in a paired computer lends it.

    - One GraphQL query per repository resolves the branch to its pull request.
    - A failing job's log comes over REST, cut to **8 KiB** as `check_log` does.
    - With no lease, the phone asks the computer as before.

    Closes #1287.
    """

    static let conversation: [GHComment] = [
        GHComment(author: hubot, when: "2h", body: "Lovely. The 60 s refresh only runs while the screen is visible, right?",
                  verdict: .approved),
        GHComment(author: monalisa, when: "1h",
                  body: "A 401 should drop the lease before asking again, or two screens race to refetch it.",
                  path: "Lease.swift · line 88", verdict: .changesRequested, replies: 1),
        GHComment(author: octocat, when: "40m", body: "Good catch, fixed in 3f2a91c. One fetch at a time now."),
    ]

    static let mine: [GHPull] = [
        GHPull(repo: repo, number: 1342, title: title, state: .open, review: .changesRequested, author: octocat,
               branch: branch, updated: "40m", failing: 2, pending: 1, passed: 5, workspace: "MacBook Pro", comments: 6),
        GHPull(repo: repo, number: 1338, title: "Keep the relay's dial keys after a restart", state: .open,
               review: .approved, author: octocat, branch: "relay-keys", updated: "3h", passed: 14,
               workspace: "studio", comments: 2),
        GHPull(repo: "sheprd-labs/herdr-site", number: 214, title: "Docs: GitHub on the phone", state: .draft,
               review: nil, author: octocat, branch: "docs-phone-github", updated: "1d", pending: 2),
        GHPull(repo: "sheprd-labs/sheprd", number: 1301, title: "Group tabs by worktree in the iPad sidebar",
               state: .merged, review: .approved, author: octocat, branch: "ipad-tabs", updated: "2d", passed: 14),
    ]

    static let requested: [GHPull] = [
        GHPull(repo: repo, number: 1345, title: "Teleport: carry checkpoints across hosts", state: .open,
               review: .required, author: hubot, branch: "teleport-checkpoints", updated: "12m", pending: 3, passed: 9,
               workspace: "build-01", comments: 1),
        GHPull(repo: "sheprd-labs/sheprd-apns", number: 57, title: "Rotate the provider token before it expires",
               state: .open, review: .required, author: monalisa, branch: "token-rotate", updated: "2h", failing: 1,
               passed: 6, comments: 3),
        GHPull(repo: "octo-org/infra", number: 902, title: "Bump relay droplets to 2 vCPU", state: .open,
               review: .required, author: dependabot, branch: "dependabot/terraform/size", updated: "5h", passed: 4),
    ]

    static let accounts: [GHAccount] = [
        GHAccount(host: "github.com", login: "octocat", name: "The Octocat", tint: .indigo,
                  lentBy: "MacBook Pro", source: "Sheprd sign-in", expires: "Renews in 6 h", others: ["studio"]),
        GHAccount(host: "github.acme-corp.com", login: "octocat-acme", name: "Octocat (ACME)", tint: .teal,
                  lentBy: "studio", source: "GitHub CLI (gh)", expires: nil),
    ]
}

// MARK: - Parts

/// A login's initial on a circle, standing in for a GitHub avatar.
struct GHAvatar: View {
    var person: GHPerson
    var size: CGFloat = 24

    var body: some View {
        Group {
            if person.login.contains("/") {
                Image(systemName: "person.2.fill").font(.system(size: size * 0.42, weight: .semibold))
            } else {
                Text(person.login.first.map { String($0).uppercased() } ?? "?")
                    .font(.system(size: size * 0.48, weight: .bold, design: .rounded))
            }
        }
        .foregroundStyle(.white)
        .frame(width: size, height: size)
        .background(person.tint.gradient, in: .circle)
    }
}

/// State as a colored capsule: Open, Draft, Merged.
struct GHStatePill: View {
    var state: GHState

    var body: some View {
        GHLabel(state.label, systemImage: state.symbol)
            .font(.caption.weight(.semibold))
            .foregroundStyle(state == .draft ? AnyShapeStyle(.secondary) : AnyShapeStyle(.white))
            .padding(.horizontal, 8).padding(.vertical, 3)
            .background(state == .draft ? AnyShapeStyle(.fill.secondary) : AnyShapeStyle(state.color.gradient), in: .capsule)
            .fixedSize()
    }
}

/// A review decision in its color.
struct GHReviewLabel: View {
    var review: GHReview

    var body: some View {
        GHLabel(review.label, systemImage: review.symbol)
            .font(.caption.weight(.medium))
            .foregroundStyle(review.color)
            .lineLimit(1)
    }
}

/// The caption on an answer the computer gave rather than GitHub.
struct GHVia: View {
    var computer = "MacBook Pro"

    var body: some View {
        GHLabel("via \(computer)", systemImage: "laptopcomputer")
            .font(.caption2)
            .foregroundStyle(.secondary)
            .lineLimit(1)
    }
}

/// Failing, pending and passed counts, as small colored numbers.
struct GHCheckCounts: View {
    var failing: Int
    var pending: Int
    var passed: Int

    var body: some View {
        HStack(spacing: 8) {
            if failing > 0 { count(failing, "xmark.circle.fill", .red) }
            if pending > 0 { count(pending, "clock.fill", .yellow) }
            if passed > 0 { count(passed, "checkmark.circle.fill", .green) }
        }
        .font(.caption.weight(.semibold).monospacedDigit())
        .fixedSize()
    }

    private func count(_ value: Int, _ symbol: String, _ color: Color) -> some View {
        HStack(spacing: 3) {
            Image(systemName: symbol).foregroundStyle(color)
            Text("\(value)").foregroundStyle(.secondary)
        }
    }
}

/// A check with its state; a failure with a log offers it to an agent.
struct GHCheckRow: View {
    var check: GHCheck
    var compact = false

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 10) {
                Image(systemName: check.symbol).foregroundStyle(check.color)
                VStack(alignment: .leading, spacing: 1) {
                    Text(check.name).lineLimit(1)
                    if !compact { Text(check.detail).font(.caption).foregroundStyle(.secondary) }
                }
                Spacer(minLength: 4)
                Image(systemName: "arrow.up.right.square").foregroundStyle(.tint)
            }
            if check.hasLog && !compact {
                Label("Read failing log", systemImage: "text.bubble")
                    .font(.subheadline).foregroundStyle(.tint)
                    .padding(.leading, 30)
            }
        }
    }
}

/// A changed file, as the Changes screen lists it.
struct GHFileRow: View {
    var file: GHFile

    var body: some View {
        let name = (file.path as NSString).lastPathComponent
        let folder = (file.path as NSString).deletingLastPathComponent
        HStack(spacing: 10) {
            Image(systemName: file.removed == 0 ? "plus.circle" : "pencil.circle")
                .foregroundStyle(file.removed == 0 ? .green : .secondary)
            VStack(alignment: .leading, spacing: 1) {
                Text(name).font(.body.monospaced()).lineLimit(1).truncationMode(.middle)
                Text(folder).font(.caption).foregroundStyle(.secondary).lineLimit(1).truncationMode(.head)
            }
            Spacer(minLength: 4)
            HStack(spacing: 6) {
                if file.added > 0 { Text("+\(file.added)").foregroundStyle(.green) }
                if file.removed > 0 { Text("−\(file.removed)").foregroundStyle(.red) }
            }
            .font(.caption.monospaced())
            .fixedSize()
        }
    }
}

/// The Uncommitted / Branch control on top of the Changes screen.
struct GHScopePicker: View {
    var body: some View {
        Section {
            Picker("Compare", selection: .constant(1)) {
                Text("Uncommitted").tag(0)
                Text("Branch").tag(1)
            }
            .pickerStyle(.segmented)
            .listRowBackground(Color.clear)
            .listRowInsets(EdgeInsets())
        }
    }
}

/// Markdown as the app's `MarkdownView` would draw this sample: paragraphs
/// and a bullet list, with inline styles.
struct GHMarkdown: View {
    var text: String

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            ForEach(Array(text.components(separatedBy: "\n\n").enumerated()), id: \.offset) { _, block in
                let lines = block.components(separatedBy: "\n")
                if lines.allSatisfy({ $0.hasPrefix("- ") }) {
                    VStack(alignment: .leading, spacing: 4) {
                        ForEach(Array(lines.enumerated()), id: \.offset) { _, line in
                            HStack(alignment: .firstTextBaseline, spacing: 8) {
                                Text("•").foregroundStyle(.secondary)
                                Text(inline(String(line.dropFirst(2))))
                            }
                        }
                    }
                } else {
                    Text(inline(block))
                }
            }
        }
        .font(.subheadline)
    }

    private func inline(_ text: String) -> AttributedString {
        (try? AttributedString(markdown: text)) ?? AttributedString(text)
    }
}

/// An inline title, as the app's pushed screens have.
extension View {
    func ghScreen(_ title: String) -> some View {
        navigationTitle(title).navigationBarTitleDisplayMode(.inline)
    }
}

/// An icon and its text side by side. A `Label` in a list row takes the
/// row's icon column, which spreads these small captions apart.
struct GHLabel: View {
    var title: String
    var systemImage: String

    init(_ title: String, systemImage: String) {
        self.title = title
        self.systemImage = systemImage
    }

    var body: some View {
        HStack(spacing: 4) {
            Image(systemName: systemImage)
            Text(title)
        }
    }
}
