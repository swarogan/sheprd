import SheprdGitHub
import SheprdRemoteCore
import SwiftUI

/// Changes > Branch's pull request: what needs you first, a red banner
/// with the failing checks and their log, then a compact summary that
/// opens the pull request. A computer's answer carries only the number and
/// the checks, so the summary then shows those, captioned "via NAME".
struct PullRequestHeader: View {
    let checks: PullRequestChecks
    /// From GitHub; nil when the computer answered.
    let pullRequest: GitHubPullRequest?
    /// Its computer's badge gives a number and page when GitHub did not
    /// answer; its repository is where the detail screen reads.
    let workspace: Home.Workspace?
    let origin: ReadOrigin?
    let sendLog: (PullRequestCheck) -> Void

    private var badge: Home.PullRequest? { workspace?.pullRequest }

    private var summary: PullRequestSummary? {
        if let pullRequest { return PullRequestSummary(pullRequest) }
        guard let number = checks.number ?? badge?.number else { return nil }
        let url = badge?.number == number ? badge?.url : nil
        return PullRequestSummary(number: number, url: url, look: PullRequestLook(tone: badge?.tone))
    }

    private var failing: [PullRequestCheck] { checks.checks.filter { $0.state == .failed } }

    var body: some View {
        if !failing.isEmpty {
            Section {
                FailingBanner(failing: failing, page: summary?.page, sendLog: sendLog)
                    .listRowBackground(Color.clear)
                    .listRowInsets(EdgeInsets())
            }
        }
        if let summary {
            Section {
                VStack(alignment: .leading, spacing: 6) {
                    HStack(spacing: 6) {
                        Image(systemName: summary.look.symbol).foregroundStyle(summary.look.color)
                            .accessibilityHidden(true)
                        Text(verbatim: "#\(summary.number)").font(.subheadline.weight(.semibold).monospacedDigit())
                        Text(summary.look.label).font(.subheadline).foregroundStyle(.secondary)
                        Spacer(minLength: 4)
                        PullRequestVia(origin: origin, direct: workspace?.github != nil)
                    }
                    if let title = summary.title, !title.isEmpty {
                        Text(title).font(.subheadline).lineLimit(2)
                    }
                }
                .accessibilityElement(children: .combine)
                review
                NavigationLink {
                    PullRequestDetailScreen(
                        target: workspace.flatMap { PullRequestTarget(workspace: $0, number: summary.number) },
                        summary: summary
                    )
                } label: {
                    Text("Open pull request")
                }
            }
        }
    }

    /// The review decision, and how the checks that are not failing stand.
    private var review: some View {
        HStack(spacing: 8) {
            if let look = pullRequest?.reviewDecision.look {
                HStack(spacing: 5) {
                    Image(systemName: look.symbol)
                    Text(look.label)
                }
                .font(.caption.weight(.medium)).foregroundStyle(look.color).lineLimit(1)
                .layoutPriority(1)
            }
            Spacer(minLength: 8)
            Text(counts).font(.caption).foregroundStyle(.secondary).lineLimit(1)
        }
    }

    private var counts: String {
        let count = { (state: PullRequestCheck.State) in checks.checks.count { $0.state == state } }
        var parts = [(count(.pending), "running"), (count(.passed), "passed"), (count(.skipped), "skipped")]
            .filter { $0.0 > 0 }.map { "\($0.0) \($0.1)" }
        // GitHub listed only the first page: the rest, failing ones
        // included, are on the pull request's page.
        if pullRequest?.moreChecks == true { parts.append("more on GitHub") }
        if parts.isEmpty { return checks.checks.isEmpty ? "No checks" : "" }
        return parts.joined(separator: " · ")
    }
}

/// The red banner: how many checks fail and which, the log of the first
/// with one (or a choice among several), and the failing check's page.
private struct FailingBanner: View {
    let failing: [PullRequestCheck]
    let page: URL?
    let sendLog: (PullRequestCheck) -> Void

    private var withLogs: [PullRequestCheck] { failing.filter { $0.job != nil } }

    /// The first failing check's page, else the pull request's.
    private var link: URL? {
        failing.lazy.compactMap { $0.url.flatMap(PullRequestLink.https) }.first ?? page
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 10) {
                Image(systemName: "xmark.octagon.fill").font(.title2).foregroundStyle(.white)
                    .accessibilityHidden(true)
                VStack(alignment: .leading, spacing: 1) {
                    Text(failing.count == 1 ? "1 check failing" : "\(failing.count) checks failing")
                        .font(.headline).foregroundStyle(.white)
                    Text(failing.map(\.name).joined(separator: " · ")).font(.caption)
                        .foregroundStyle(.white.opacity(0.85)).lineLimit(1)
                }
            }
            HStack(spacing: 8) {
                if let first = withLogs.first {
                    Menu {
                        ForEach(withLogs) { check in Button(check.name) { sendLog(check) } }
                    } label: {
                        readLabel
                    } primaryAction: {
                        sendLog(first)
                    }
                    .menuIndicator(.hidden)
                    .buttonStyle(.glass)
                    .accessibilityHint(withLogs.count > 1 ? "Touch and hold to choose another failing check" : "")
                    if let link { linkButton(link) }
                } else if let link {
                    Link(destination: link) {
                        Label("Open on GitHub", systemImage: "arrow.up.right.square").font(.subheadline.weight(.semibold))
                            .frame(maxWidth: .infinity)
                    }
                    .buttonStyle(.glass)
                }
            }
        }
        .padding(14)
        .background(Color.red.gradient, in: .rect(cornerRadius: 22))
    }

    private var readLabel: some View {
        Label("Read failing log", systemImage: "text.bubble").font(.subheadline.weight(.semibold))
            .frame(maxWidth: .infinity)
    }

    private func linkButton(_ link: URL) -> some View {
        Link(destination: link) {
            Image(systemName: "arrow.up.right.square").font(.subheadline.weight(.semibold))
        }
        .buttonStyle(.glass)
        .accessibilityLabel("Open on GitHub")
    }
}
