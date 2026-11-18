import SwiftUI

// A pull request, read-only: description, reviewers, conversation and
// review comments, and the files it changes (opening the existing diff).
// A: one grouped list, a section for each.
// B: Overview, Conversation, Files under a segmented control.
// C: a timeline, as GitHub's conversation tab reads, reviewers as chips.
// D: a grid of counts on top, review comments grouped by file.
// State 0 is the top of the screen; state 1 the rest (B: Conversation).

private struct DetailFrame<Content: View>: View {
    var state: Int
    @ViewBuilder var content: Content

    var body: some View {
        NavigationStack {
            ScrollViewReader { proxy in
                List {
                    content
                    Color.clear.frame(height: 1).listRowBackground(Color.clear).id("end")
                }
                .listSectionSpacing(14)
                .task {
                    guard state == 1 else { return }
                    try? await Task.sleep(for: .milliseconds(300))
                    proxy.scrollTo("end", anchor: .bottom)
                }
            }
            .ghScreen("#1342")
                .toolbar {
                    ToolbarItem(placement: .topBarTrailing) {
                        Button {} label: { Image(systemName: "safari") }
                    }
                }
        }
    }
}

/// The title block every variant starts with.
private struct TitleBlock: View {
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 8) {
                GHStatePill(state: .open)
                Text(verbatim: "sheprd-labs/sheprd").font(.caption.monospaced()).foregroundStyle(.secondary)
                    .lineLimit(1)
            }
            Text(GH.title).font(.title3.weight(.semibold)).fixedSize(horizontal: false, vertical: true)
            HStack(spacing: 6) {
                GHAvatar(person: GH.octocat, size: 18)
                Text("octocat · \(GH.branch) → main").font(.caption).foregroundStyle(.secondary)
                    .lineLimit(1).truncationMode(.middle)
            }
        }
        .padding(.vertical, 4)
    }
}

private struct ReviewerRow: View {
    var reviewer: GHReviewer

    var body: some View {
        HStack(spacing: 10) {
            GHAvatar(person: reviewer.person, size: 28)
            Text(reviewer.person.login).lineLimit(1)
            Spacer(minLength: 4)
            if let verdict = reviewer.verdict {
                GHReviewLabel(review: verdict)
            } else {
                Text("Requested").font(.caption).foregroundStyle(.secondary)
            }
        }
    }
}

private struct CommentCard: View {
    var comment: GHComment

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 8) {
                GHAvatar(person: comment.author, size: 22)
                Text(comment.author.login).font(.subheadline.weight(.semibold)).lineLimit(1)
                Text(comment.when).font(.caption).foregroundStyle(.secondary)
                Spacer(minLength: 4)
                if let verdict = comment.verdict {
                    Image(systemName: verdict.symbol).foregroundStyle(verdict.color).font(.caption)
                }
            }
            if let path = comment.path {
                GHLabel(path, systemImage: "doc.text").font(.caption.monospaced()).foregroundStyle(.secondary)
                    .lineLimit(1)
            }
            Text(comment.body).font(.subheadline).fixedSize(horizontal: false, vertical: true)
            if comment.replies > 0 {
                Text(comment.replies == 1 ? "1 reply" : "\(comment.replies) replies").font(.caption)
                    .foregroundStyle(.tint)
            }
        }
        .padding(.vertical, 2)
    }
}

private struct FilesSection: View {
    var body: some View {
        Section {
            ForEach(GH.files) { file in NavigationLink {} label: { GHFileRow(file: file) } }
        } header: {
            HStack {
                Text("5 files changed")
                Spacer()
                Text("+619 −50").font(.caption.monospaced()).textCase(nil)
            }
        }
    }
}

// MARK: A

struct PRDetailSections: View {
    var state: Int

    var body: some View {
        DetailFrame(state: state) {
            Section { TitleBlock() }
            Section("Description") { GHMarkdown(text: GH.body).padding(.vertical, 4) }
            Section("Reviewers") { ForEach(GH.reviewers) { ReviewerRow(reviewer: $0) } }
            Section("Conversation") { ForEach(GH.conversation) { CommentCard(comment: $0) } }
            FilesSection()
        }
    }
}

// MARK: B

struct PRDetailTabs: View {
    var state: Int

    var body: some View {
        NavigationStack {
            List {
                Section { TitleBlock() }
                Section {
                    Picker("Show", selection: .constant(state)) {
                        Text("Overview").tag(0)
                        Text("Conversation").tag(1)
                        Text("Files").tag(2)
                    }
                    .pickerStyle(.segmented)
                    .listRowBackground(Color.clear)
                    .listRowInsets(EdgeInsets())
                }
                if state == 0 {
                    Section { GHMarkdown(text: GH.body).padding(.vertical, 4) }
                    Section("Reviewers") { ForEach(GH.reviewers) { ReviewerRow(reviewer: $0) } }
                    Section("Checks") {
                        HStack {
                            Text("8 checks")
                            Spacer()
                            GHCheckCounts(failing: 2, pending: 1, passed: 5)
                        }
                    }
                } else {
                    ForEach(GH.conversation) { comment in
                        Section { CommentCard(comment: comment) }
                    }
                    Section {
                        Text("Replying and reviewing come later. Open GitHub to answer.")
                            .font(.footnote).foregroundStyle(.secondary)
                            .listRowBackground(Color.clear)
                    }
                }
            }
            .listSectionSpacing(12)
            .ghScreen("#1342")
        }
    }
}

// MARK: C

struct PRDetailTimeline: View {
    var state: Int

    var body: some View {
        DetailFrame(state: state) {
            Section {
                TitleBlock()
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 6) {
                        ForEach(GH.reviewers) { reviewer in
                            HStack(spacing: 5) {
                                GHAvatar(person: reviewer.person, size: 18)
                                Text(reviewer.person.login).font(.caption.weight(.medium))
                                if let verdict = reviewer.verdict {
                                    Image(systemName: verdict.symbol).font(.caption2).foregroundStyle(verdict.color)
                                }
                            }
                            .padding(.leading, 3).padding(.trailing, 8).padding(.vertical, 3)
                            .background(.fill.tertiary, in: .capsule)
                        }
                    }
                }
            }
            Section {
                TimelineItem(person: GH.octocat, title: "octocat opened this", when: "5h", last: false) {
                    GHMarkdown(text: GH.body)
                }
                TimelineItem(person: GH.hubot, title: "hubot approved", when: "2h", last: false,
                             symbol: "checkmark.seal.fill", tint: .green) {
                    Text(GH.conversation[0].body).font(.subheadline)
                }
                TimelineItem(person: GH.monalisa, title: "monalisa requested changes", when: "1h", last: false,
                             symbol: "exclamationmark.bubble.fill", tint: .red) {
                    VStack(alignment: .leading, spacing: 6) {
                        GHLabel("Lease.swift · line 88", systemImage: "doc.text").font(.caption.monospaced())
                            .foregroundStyle(.secondary)
                        Text(GH.conversation[1].body).font(.subheadline)
                    }
                    .padding(10)
                    .background(.fill.quinary, in: .rect(cornerRadius: 12))
                }
                TimelineItem(person: GH.octocat, title: "octocat commented", when: "40m", last: true) {
                    Text(GH.conversation[2].body).font(.subheadline)
                }
            } header: {
                Text("Conversation")
            }
            FilesSection()
        }
    }
}

private struct TimelineItem<Content: View>: View {
    var person: GHPerson
    var title: String
    var when: String
    var last: Bool
    var symbol: String? = nil
    var tint: Color = .secondary
    @ViewBuilder var content: Content

    var body: some View {
        HStack(alignment: .top, spacing: 10) {
            VStack(spacing: 0) {
                GHAvatar(person: person, size: 26)
                if !last { Rectangle().fill(.quaternary).frame(width: 2).frame(maxHeight: .infinity) }
            }
            VStack(alignment: .leading, spacing: 6) {
                HStack(spacing: 5) {
                    if let symbol { Image(systemName: symbol).foregroundStyle(tint).font(.caption) }
                    Text(title).font(.subheadline.weight(.semibold)).lineLimit(1)
                    Spacer(minLength: 4)
                    Text(when).font(.caption).foregroundStyle(.secondary)
                }
                content
            }
            .padding(.bottom, last ? 0 : 6)
        }
    }
}

// MARK: D

struct PRDetailGrid: View {
    var state: Int

    var body: some View {
        DetailFrame(state: state) {
            Section { TitleBlock() }
            Section {
                LazyVGrid(columns: [GridItem(.flexible(), spacing: 10), GridItem(.flexible())], spacing: 10) {
                    Tile(symbol: "xmark.circle.fill", tint: .red, value: "2 failing", detail: "of 8 checks")
                    Tile(symbol: "exclamationmark.bubble.fill", tint: .red, value: "Changes", detail: "1 of 3 reviewers")
                    Tile(symbol: "text.bubble.fill", tint: .blue, value: "6 comments", detail: "2 on code")
                    Tile(symbol: "doc.on.doc.fill", tint: .gray, value: "5 files", detail: "+619 −50")
                }
                .listRowBackground(Color.clear)
                .listRowInsets(EdgeInsets())
            }
            Section {
                VStack(alignment: .leading, spacing: 6) {
                    Text("The phone now asks GitHub itself, with a sign-in a paired computer lends it.")
                        .font(.subheadline).lineLimit(3)
                    Text("Show more").font(.subheadline.weight(.medium)).foregroundStyle(.tint)
                }
            } header: {
                Text("Description")
            }
            Section {
                ForEach(GH.reviewers) { ReviewerRow(reviewer: $0) }
            } header: {
                Text("Reviewers")
            }
            Section {
                ForEach([GH.conversation[0], GH.conversation[2]]) { CommentCard(comment: $0) }
            } header: {
                Text("Conversation")
            }
            Section {
                VStack(alignment: .leading, spacing: 0) {
                    Text(verbatim: "  87  guard lease.isValid else {").foregroundStyle(.secondary)
                    Text(verbatim: "+ 88      try await refetch(host)").background(Color.green.opacity(0.18))
                }
                .font(.system(.caption, design: .monospaced))
                .lineLimit(1)
                CommentCard(comment: GHComment(author: GH.monalisa, when: "1h", body: GH.conversation[1].body,
                                               verdict: .changesRequested, replies: 1))
            } header: {
                Text(verbatim: "Lease.swift").textCase(nil).font(.subheadline.monospaced())
            }
            FilesSection()
        }
    }
}

private struct Tile: View {
    var symbol: String
    var tint: Color
    var value: String
    var detail: String

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Image(systemName: symbol).font(.title3).foregroundStyle(tint).frame(height: 26)
            Text(value).font(.headline).lineLimit(1)
            Text(detail).font(.caption).foregroundStyle(.secondary).lineLimit(1)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(12)
        .background(Color(.secondarySystemGroupedBackground), in: .rect(cornerRadius: 18))
    }
}
