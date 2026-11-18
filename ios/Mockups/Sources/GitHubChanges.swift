import SwiftUI

// Changes > Branch with the branch's pull request on top.
// A: a header card, then failing checks first, the rest folded.
// B: plain grouped rows, as Settings has: state, review, checks.
// C: what needs you first, a red banner with the failing log, then the PR.
// D: one bar for every check and the reviewers' faces, checks folded.
// State 0 read GitHub directly; state 1 is a draft the computer answered for,
// with the "via MacBook Pro" caption.

private struct ChangesFrame<Header: View>: View {
    @ViewBuilder var header: Header

    var body: some View {
        NavigationStack {
            List {
                GHScopePicker()
                header
                Section("5 files against main") {
                    ForEach(GH.files) { GHFileRow(file: $0) }
                }
            }
            .listSectionSpacing(14)
            .ghScreen("Changes")
        }
    }
}

private extension Int {
    var viaComputer: Bool { self == 1 }
    var pullState: GHState { self == 1 ? .draft : .open }
}

// MARK: A

struct PRHeadCard: View {
    var state: Int

    var body: some View {
        ChangesFrame {
            Section {
                VStack(alignment: .leading, spacing: 10) {
                    HStack(spacing: 8) {
                        GHStatePill(state: state.pullState)
                        Text(verbatim: "#1342").font(.subheadline.weight(.semibold).monospacedDigit())
                            .foregroundStyle(.secondary)
                        Spacer()
                        if state.viaComputer { GHVia() } else {
                            Image(systemName: "arrow.up.right.square").foregroundStyle(.tint)
                        }
                    }
                    Text(GH.title).font(.headline).fixedSize(horizontal: false, vertical: true)
                    HStack(spacing: 6) {
                        GHAvatar(person: GH.octocat, size: 18)
                        Text("octocat wants to merge into main").font(.caption).foregroundStyle(.secondary)
                            .lineLimit(1)
                    }
                    Divider()
                    HStack {
                        GHReviewLabel(review: state.viaComputer ? .required : .changesRequested)
                        Spacer(minLength: 8)
                        GHCheckCounts(failing: 2, pending: 1, passed: 5)
                    }
                }
                .padding(.vertical, 4)
                NavigationLink {} label: { Label("Conversation and reviews", systemImage: "text.bubble") }
            }
            Section {
                ForEach(GH.failing) { GHCheckRow(check: $0) }
                ForEach(GH.pending) { GHCheckRow(check: $0) }
                DisclosureGroup("\(GH.settled.count) passed or skipped") {}
            } header: {
                Text("Checks")
            }
        }
    }
}

// MARK: B

struct PRHeadRows: View {
    var state: Int

    var body: some View {
        ChangesFrame {
            Section {
                VStack(alignment: .leading, spacing: 4) {
                    Text(GH.title).font(.body.weight(.semibold)).fixedSize(horizontal: false, vertical: true)
                    Text(verbatim: "#1342 · \(GH.branch) → main").font(.caption.monospaced())
                        .foregroundStyle(.secondary).lineLimit(1).truncationMode(.middle)
                }
                .padding(.vertical, 2)
                LabeledContent("State") {
                    GHLabel(state.pullState.label, systemImage: state.pullState.symbol)
                        .foregroundStyle(state.pullState.color)
                }
                LabeledContent("Review") {
                    let review: GHReview = state.viaComputer ? .required : .changesRequested
                    Text(review.label).foregroundStyle(review.color)
                }
                NavigationLink {} label: {
                    LabeledContent("Checks") {
                        Text("2 failing, 1 running").foregroundStyle(.red)
                    }
                }
                NavigationLink {} label: { Text("Details, reviewers, and comments") }
            } header: {
                HStack {
                    Text("Pull request")
                    Spacer()
                    if state.viaComputer { GHVia().textCase(nil) }
                }
            }
            Section("Failing") {
                ForEach(GH.failing) { GHCheckRow(check: $0) }
            }
        }
    }
}

// MARK: C

struct PRHeadBanner: View {
    var state: Int

    var body: some View {
        ChangesFrame {
            Section {
                VStack(alignment: .leading, spacing: 10) {
                    HStack(spacing: 10) {
                        Image(systemName: "xmark.octagon.fill").font(.title2).foregroundStyle(.white)
                        VStack(alignment: .leading, spacing: 1) {
                            Text("2 checks failing").font(.headline).foregroundStyle(.white)
                            Text("clippy (linux, all) · test (windows, default)").font(.caption)
                                .foregroundStyle(.white.opacity(0.85)).lineLimit(1)
                        }
                    }
                    HStack(spacing: 8) {
                        Button {} label: {
                            Label("Read failing log", systemImage: "text.bubble").font(.subheadline.weight(.semibold))
                                .frame(maxWidth: .infinity)
                        }
                        .buttonStyle(.glass)
                        Button {} label: {
                            Image(systemName: "arrow.up.right.square").font(.subheadline.weight(.semibold))
                        }
                        .buttonStyle(.glass)
                    }
                }
                .padding(14)
                .background(Color.red.gradient, in: .rect(cornerRadius: 22))
                .listRowBackground(Color.clear)
                .listRowInsets(EdgeInsets())
            }
            Section {
                VStack(alignment: .leading, spacing: 6) {
                    HStack(spacing: 6) {
                        Image(systemName: state.pullState.symbol).foregroundStyle(state.pullState.color)
                        Text(verbatim: "#1342").font(.subheadline.weight(.semibold).monospacedDigit())
                        Text(state.pullState.label).font(.subheadline).foregroundStyle(.secondary)
                        Spacer()
                        if state.viaComputer { GHVia() }
                    }
                    Text(GH.title).font(.subheadline).lineLimit(2)
                }
                HStack {
                    GHReviewLabel(review: state.viaComputer ? .required : .changesRequested)
                    Spacer()
                    Text("1 running · 5 passed").font(.caption).foregroundStyle(.secondary)
                }
                NavigationLink {} label: { Text("Open pull request") }
            }
        }
    }
}

// MARK: D

struct PRHeadBar: View {
    var state: Int

    var body: some View {
        ChangesFrame {
            Section {
                VStack(alignment: .leading, spacing: 12) {
                    HStack(alignment: .top) {
                        VStack(alignment: .leading, spacing: 3) {
                            Text(verbatim: "sheprd-labs/sheprd #1342").font(.caption.monospaced())
                                .foregroundStyle(.secondary)
                            Text(GH.title).font(.headline).fixedSize(horizontal: false, vertical: true)
                        }
                        Spacer(minLength: 8)
                        GHStatePill(state: state.pullState)
                    }
                    CheckBar(failing: 2, pending: 1, passed: 4, skipped: 1)
                    HStack(spacing: 8) {
                        HStack(spacing: -6) {
                            ForEach(GH.reviewers) { reviewer in
                                GHAvatar(person: reviewer.person, size: 24)
                                    .overlay(alignment: .bottomTrailing) {
                                        if let verdict = reviewer.verdict {
                                            Image(systemName: verdict == .approved ? "checkmark.circle.fill" : "minus.circle.fill")
                                                .font(.system(size: 10)).foregroundStyle(verdict.color)
                                                .background(.background, in: .circle)
                                        }
                                    }
                            }
                        }
                        GHReviewLabel(review: state.viaComputer ? .required : .changesRequested)
                        Spacer(minLength: 4)
                    }
                    if state.viaComputer { GHVia() }
                }
                .padding(.vertical, 4)
                DisclosureGroup(isExpanded: .constant(true)) {
                    ForEach(GH.failing) { GHCheckRow(check: $0) }
                    GHCheckRow(check: GH.pending[0], compact: true)
                } label: {
                    Label("8 checks", systemImage: "checklist")
                }
            }
        }
    }
}

/// Every check as one segmented bar, failures first.
private struct CheckBar: View {
    var failing: Int
    var pending: Int
    var passed: Int
    var skipped: Int

    var body: some View {
        VStack(alignment: .leading, spacing: 5) {
            GeometryReader { proxy in
                let total = CGFloat(failing + pending + passed + skipped)
                let unit = (proxy.size.width - 6) / max(total, 1)
                HStack(spacing: 2) {
                    Color.red.frame(width: unit * CGFloat(failing))
                    Color.yellow.frame(width: unit * CGFloat(pending))
                    Color.green.frame(width: unit * CGFloat(passed))
                    Color.gray.opacity(0.4).frame(width: unit * CGFloat(skipped))
                }
            }
            .frame(height: 6)
            .clipShape(.capsule)
            Text("\(failing) failing · \(pending) running · \(passed) passed · \(skipped) skipped")
                .font(.caption).foregroundStyle(.secondary).lineLimit(1).minimumScaleFactor(0.85)
        }
    }
}
