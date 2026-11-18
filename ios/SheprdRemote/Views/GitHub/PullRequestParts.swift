import SheprdGitHub
import SheprdRemoteCore
import SwiftUI

/// What a pull request header and detail screen know before or without
/// GitHub: everything from a direct read, or only the number and page a
/// computer's badge carries.
struct PullRequestSummary: Hashable {
    var number: UInt64
    var title: String?
    var url: String?
    var look: PullRequestLook
    var author: String?

    init(number: UInt64, title: String? = nil, url: String? = nil, look: PullRequestLook = .unknown, author: String? = nil) {
        self.number = number
        self.title = title
        self.url = url
        self.look = look
        self.author = author
    }

    init(_ pullRequest: GitHubPullRequest) {
        self.init(number: pullRequest.number, title: pullRequest.title, url: pullRequest.url,
                  look: PullRequestLook(pullRequest.state, draft: pullRequest.isDraft), author: pullRequest.author)
    }

    /// Its page, when it is one this phone opens.
    var page: URL? { url.flatMap(PullRequestLink.https) }
}

/// A pull request's lifecycle as icon, word, and color.
enum PullRequestLook: Hashable {
    case open, draft, merged, closed
    /// A computer's answer says no more than that there is one.
    case unknown

    init(_ state: GitHubPullRequest.State, draft: Bool) {
        self = switch state {
        case .open: draft ? .draft : .open
        case .merged: .merged
        case .closed: .closed
        case .unknown: .unknown
        }
    }

    init(tone: Home.PullRequest.Tone?) {
        self = tone == .merged ? .merged : .unknown
    }

    var label: String {
        switch self {
        case .open: "Open"
        case .draft: "Draft"
        case .merged: "Merged"
        case .closed: "Closed"
        case .unknown: "Pull request"
        }
    }

    var symbol: String {
        switch self {
        case .open, .unknown: "arrow.triangle.pull"
        case .draft: "pencil.circle"
        case .merged: "arrow.triangle.merge"
        case .closed: "xmark.circle"
        }
    }

    var color: Color {
        switch self {
        case .open: .green
        case .draft, .unknown: .secondary
        case .merged: .purple
        case .closed: .red
        }
    }
}

extension GitHubPullRequest.ReviewDecision {
    /// Its label; nil when nobody needs to review.
    var look: (label: String, symbol: String, color: Color)? {
        switch self {
        case .approved: ("Approved", "checkmark.seal.fill", .green)
        case .changesRequested: ("Changes requested", "exclamationmark.bubble.fill", .red)
        case .reviewRequired: ("Review required", "eye", .orange)
        case .none: nil
        }
    }
}

extension PullRequestDetail.Verdict {
    var symbol: String {
        switch self {
        case .approved: "checkmark.seal.fill"
        case .changesRequested: "exclamationmark.bubble.fill"
        case .commented: "text.bubble"
        case .dismissed: "xmark.bubble"
        case .pending: "clock"
        }
    }

    var color: Color {
        switch self {
        case .approved: .green
        case .changesRequested: .red
        case .commented, .dismissed, .pending: .secondary
        }
    }
}

/// A capsule with the state's icon and word.
struct PullRequestStatePill: View {
    var look: PullRequestLook

    var body: some View {
        // An HStack, not a Label: a list row lays a Label's icon out in a
        // column of its own.
        HStack(spacing: 4) {
            Image(systemName: look.symbol)
            Text(look.label)
        }
        .font(.caption.weight(.semibold))
        .foregroundStyle(look == .open ? .white : look.color)
        .padding(.horizontal, 9).padding(.vertical, 4)
        .background(look == .open ? AnyShapeStyle(Color.green.gradient) : AnyShapeStyle(.fill.tertiary), in: .capsule)
    }
}

/// A person's initial on a color of their own, or a team's icon. No
/// picture is fetched: an avatar's address is GitHub's, not the token's.
struct PullRequestAvatar: View {
    var login: String?
    var team = false
    var size: CGFloat = 24

    var body: some View {
        Group {
            if team {
                Image(systemName: "person.2.fill").font(.system(size: size * 0.42, weight: .semibold))
            } else {
                Text(login?.first.map { String($0).uppercased() } ?? "?")
                    .font(.system(size: size * 0.48, weight: .bold, design: .rounded))
            }
        }
        .foregroundStyle(.white)
        .frame(width: size, height: size)
        .background(tint.gradient, in: .circle)
        .accessibilityHidden(true)
    }

    private var tint: Color {
        guard let login, !team else { return .gray }
        let palette: [Color] = [.indigo, .teal, .pink, .orange, .blue, .purple, .mint, .red, .cyan, .brown]
        let sum = login.lowercased().unicodeScalars.reduce(0) { ($0 &* 31 &+ Int($1.value)) & 0xFFFF }
        return palette[sum % palette.count]
    }
}

/// "via NAME" with a laptop, when a computer answered where GitHub could
/// have.
struct PullRequestVia: View {
    var origin: ReadOrigin?
    var direct: Bool

    var body: some View {
        if direct, case let .computer(name)? = origin, !name.isEmpty {
            HStack(spacing: 4) {
                Image(systemName: "laptopcomputer")
                Text("via \(name)")
            }
            .font(.caption2).foregroundStyle(.secondary).lineLimit(1)
            .accessibilityElement(children: .combine)
        }
    }
}

enum PullRequestLink {
    /// `text` as an address this phone opens: `https` only, as for checks.
    static func https(_ text: String) -> URL? {
        guard text.utf8.count <= 2048, let url = URL(string: text), url.scheme?.lowercased() == "https", url.host() != nil
        else { return nil }
        return url
    }

    /// How long ago, in a few characters: "now", "40m", "5h", "3d", or the
    /// date past a month.
    static func age(_ date: Date?, now: Date = .now) -> String {
        guard let date else { return "" }
        let seconds = max(now.timeIntervalSince(date), 0)
        switch seconds {
        case ..<60: return "now"
        case ..<3600: return "\(Int(seconds / 60))m"
        case ..<86400: return "\(Int(seconds / 3600))h"
        case ..<(30 * 86400): return "\(Int(seconds / 86400))d"
        default: return date.formatted(.dateTime.month(.abbreviated).day())
        }
    }
}
