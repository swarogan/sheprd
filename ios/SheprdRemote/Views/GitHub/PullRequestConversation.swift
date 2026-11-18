import SheprdGitHub
import SheprdRemoteCore
import SwiftUI

/// A pull request's conversation as GitHub's tab reads it: the description
/// first, then comments, reviews with their file comments, pushes, and
/// events, each beside its author's initial on a line that joins them.
struct PullRequestConversation: View {
    let detail: PullRequestDetail

    var body: some View {
        Section {
            TimelineRow(actor: detail.author, title: "\(detail.author ?? "Someone") opened this",
                        date: detail.createdAt, last: detail.timeline.isEmpty && detail.earlierItems == 0) {
                if detail.body.isEmpty {
                    Text("No description provided.").font(.subheadline).foregroundStyle(.secondary).italic()
                } else {
                    MarkdownView(text: detail.body).font(.subheadline)
                }
            }
            if detail.earlierItems > 0 {
                Label(detail.earlierItems == 1 ? "1 earlier item is on GitHub" : "\(detail.earlierItems) earlier items are on GitHub",
                      systemImage: "ellipsis")
                    .font(.footnote).foregroundStyle(.secondary)
            }
            ForEach(detail.timeline) { item in
                TimelineItemRow(item: item, last: item.id == detail.timeline.last?.id)
            }
        } header: {
            Text("Conversation")
        }
    }
}

private struct TimelineItemRow: View {
    let item: PullRequestDetail.TimelineItem
    let last: Bool

    private var who: String { item.actor ?? "Someone" }

    var body: some View {
        switch item.kind {
        case let .comment(body):
            TimelineRow(actor: item.actor, title: "\(who) commented", date: item.date, last: last) {
                MarkdownView(text: body).font(.subheadline)
            }
        case let .review(verdict, body, comments, more):
            TimelineRow(actor: item.actor, title: title(verdict), date: item.date, last: last,
                        symbol: verdict.symbol, tint: verdict.color) {
                VStack(alignment: .leading, spacing: 8) {
                    if !body.isEmpty { MarkdownView(text: body).font(.subheadline) }
                    ForEach(Array(comments.enumerated()), id: \.offset) { _, comment in ReviewCommentCard(comment: comment) }
                    if more > 0 {
                        Text(more == 1 ? "1 more comment on GitHub" : "\(more) more comments on GitHub")
                            .font(.caption).foregroundStyle(.secondary)
                    }
                }
            }
        case let .reviewRequested(reviewer):
            event("eye", "\(who) asked \(reviewer) to review")
        case let .commit(oid, headline):
            TimelineRow(actor: item.actor, title: "\(who) pushed \(oid)", date: item.date, last: last,
                        symbol: "arrow.up.circle", compact: true) {
                if !headline.isEmpty { Text(headline).font(.subheadline).lineLimit(2) }
            }
        case let .forcePushed(oid):
            event("arrow.uturn.up.circle", oid.map { "\(who) force-pushed to \($0)" } ?? "\(who) force-pushed")
        case .readyForReview:
            event("eye.circle", "\(who) marked this ready for review")
        case .closed:
            event("xmark.circle", "\(who) closed this", tint: .red)
        case .reopened:
            event("arrow.triangle.pull", "\(who) reopened this", tint: .green)
        case .merged:
            event("arrow.triangle.merge", "\(who) merged this", tint: .purple)
        }
    }

    private func event(_ symbol: String, _ title: String, tint: Color = .secondary) -> some View {
        TimelineRow(actor: item.actor, title: title, date: item.date, last: last, symbol: symbol, tint: tint, compact: true) {
            EmptyView()
        }
    }

    private func title(_ verdict: PullRequestDetail.Verdict) -> String {
        switch verdict {
        case .approved: "\(who) approved"
        case .changesRequested: "\(who) requested changes"
        case .commented: "\(who) reviewed"
        case .dismissed: "\(who)'s review was dismissed"
        case .pending: "\(who) started a review"
        }
    }
}

/// A comment on a line of a file: where, then what.
private struct ReviewCommentCard: View {
    let comment: PullRequestDetail.ReviewComment

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 4) {
                Image(systemName: "doc.text")
                Text(location).lineLimit(1).truncationMode(.head)
            }
            .font(.caption.monospaced()).foregroundStyle(.secondary)
            MarkdownView(text: comment.body).font(.subheadline)
        }
        .padding(10)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(.fill.quinary, in: .rect(cornerRadius: 12))
    }

    private var location: String {
        let name = (comment.path as NSString).lastPathComponent
        return comment.line.map { "\(name) · line \($0)" } ?? name
    }
}

/// An avatar on the timeline's line, a title and age, and what was said.
private struct TimelineRow<Content: View>: View {
    var actor: String?
    var title: String
    var date: Date?
    var last: Bool
    var symbol: String? = nil
    var tint: Color = .secondary
    /// An event, not something said: a smaller avatar.
    var compact = false
    @ViewBuilder var content: Content

    var body: some View {
        HStack(alignment: .top, spacing: 10) {
            VStack(spacing: 0) {
                PullRequestAvatar(login: actor, size: compact ? 20 : 26)
                    .frame(width: 26)
                if !last { Rectangle().fill(.quaternary).frame(width: 2).frame(maxHeight: .infinity) }
            }
            VStack(alignment: .leading, spacing: 6) {
                HStack(spacing: 5) {
                    if let symbol {
                        Image(systemName: symbol).foregroundStyle(tint).font(.caption).accessibilityHidden(true)
                    }
                    Text(title).font(compact ? .footnote : .subheadline.weight(.semibold))
                        .foregroundStyle(compact ? .secondary : .primary).lineLimit(compact ? 2 : 1)
                    Spacer(minLength: 4)
                    Text(PullRequestLink.age(date)).font(.caption).foregroundStyle(.secondary)
                }
                .accessibilityElement(children: .combine)
                content
            }
            .padding(.bottom, last ? 0 : 6)
        }
    }
}
