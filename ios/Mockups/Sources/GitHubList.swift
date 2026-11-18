import SwiftUI

// Pull requests in the Workspaces tab: mine, and those asking for my review,
// including repositories no computer has a workspace for.
// A: Workspaces | Pull requests switch on top of the tab; two sections.
// B: a Pull requests section above the computers, opening the full list.
// C: Mine / Review requested picker, pull requests under their repositories.
// D: review requests as cards to act on, mine as compact rows.
// State 1: B's full list opened; A, C and D show Review requested first.

enum GHTab: Hashable { case home, workspaces, settings }

/// The app's three tabs around a mockup screen.
struct GHTabs<Content: View>: View {
    var selected: GHTab
    @ViewBuilder var content: Content

    var body: some View {
        TabView(selection: .constant(selected)) {
            Tab("Home", systemImage: "house", value: GHTab.home) { Text("Home") }
            Tab("Workspaces", systemImage: "square.stack.3d.up", value: GHTab.workspaces) {
                if selected == .workspaces { content } else { Text("Workspaces") }
            }
            Tab("Settings", systemImage: "gearshape", value: GHTab.settings) {
                if selected == .settings { content } else { Text("Settings") }
            }
        }
    }
}

/// A pull request's row: repository and number, title, and its standing.
private struct PullRow: View {
    var pull: GHPull
    var showsAuthor = false

    var body: some View {
        VStack(alignment: .leading, spacing: 5) {
            HStack(spacing: 6) {
                Image(systemName: pull.state.symbol).font(.caption.weight(.semibold)).foregroundStyle(pull.state.color)
                Text(verbatim: "\(pull.repo) #\(pull.number)").font(.caption.monospaced())
                    .foregroundStyle(.secondary).lineLimit(1).truncationMode(.middle)
                Spacer(minLength: 4)
                Text(pull.updated).font(.caption).foregroundStyle(.secondary)
            }
            Text(pull.title).lineLimit(2)
            HStack(spacing: 8) {
                if showsAuthor {
                    GHAvatar(person: pull.author, size: 16)
                    Text(pull.author.login).font(.caption).foregroundStyle(.secondary).lineLimit(1)
                } else if let review = pull.review {
                    GHReviewLabel(review: review)
                } else {
                    Text(pull.state.label).font(.caption).foregroundStyle(.secondary)
                }
                Spacer(minLength: 4)
                GHCheckCounts(failing: pull.failing, pending: pull.pending, passed: pull.passed)
            }
            WhereLine(pull: pull)
        }
        .padding(.vertical, 2)
    }
}

/// Where the branch is checked out, or that no computer has it.
private struct WhereLine: View {
    var pull: GHPull

    var body: some View {
        Group {
            if let computer = pull.workspace {
                GHLabel("Workspace on \(computer)", systemImage: "laptopcomputer")
            } else {
                GHLabel("No workspace on any computer", systemImage: "icloud")
            }
        }
        .font(.caption2)
        .foregroundStyle(pull.workspace == nil ? AnyShapeStyle(.tertiary) : AnyShapeStyle(.secondary))
        .lineLimit(1)
    }
}

private struct Searching<Content: View>: View {
    var path: [Int] = []
    @ViewBuilder var content: Content

    var body: some View {
        NavigationStack(path: .constant(path)) {
            content
                .searchable(text: .constant(""), placement: .navigationBarDrawer(displayMode: .always),
                            prompt: "Workspaces, chats, terminals")
                .navigationBarTitleDisplayMode(.inline)
                .toolbar {
                    ToolbarItem(placement: .topBarTrailing) { Button {} label: { Image(systemName: "plus") } }
                }
        }
    }
}

// MARK: A

struct PRListSwitch: View {
    var state: Int

    var body: some View {
        GHTabs(selected: .workspaces) {
            Searching {
                List {
                    Section {
                        Picker("Show", selection: .constant(1)) {
                            Text("Workspaces").tag(0)
                            Text("Pull Requests").tag(1)
                        }
                        .pickerStyle(.segmented)
                        .listRowBackground(Color.clear)
                        .listRowInsets(EdgeInsets())
                    }
                    if state == 1 { requested; mine } else { mine; requested }
                }
                .listSectionSpacing(14)
            }
        }
    }

    private var mine: some View {
        Section("Mine") { ForEach(GH.mine) { PullRow(pull: $0) } }
    }

    private var requested: some View {
        Section("Review requested") { ForEach(GH.requested) { PullRow(pull: $0, showsAuthor: true) } }
    }
}

// MARK: B

struct PRListSection: View {
    var state: Int

    var body: some View {
        GHTabs(selected: .workspaces) {
            overview
        }
    }

    /// State 1 opens the full list as if "All pull requests" was tapped.
    private var overview: some View {
        Searching(path: state == 1 ? [1] : []) {
            List {
                Section {
                    ForEach(GH.mine.prefix(3)) { CompactPullRow(pull: $0) }
                    NavigationLink {} label: { Text("All pull requests").foregroundStyle(.tint) }
                } header: {
                    VStack(alignment: .leading, spacing: 10) {
                        Label("Pull requests", systemImage: "arrow.triangle.pull").font(.headline)
                            .foregroundStyle(.primary)
                        HStack(spacing: 8) {
                            Chip(title: "Mine", count: 4, on: true)
                            Chip(title: "Review requested", count: 3, on: false)
                        }
                    }
                    .textCase(nil)
                    .padding(.bottom, 4)
                }
                Section {
                    ComputerStub(name: "MacBook Pro", workspaces: ["phone-github-reads", "main"])
                } header: {
                    Label("MacBook Pro", systemImage: "laptopcomputer").font(.headline)
                        .foregroundStyle(.primary).textCase(nil)
                }
            }
            .listSectionSpacing(14)
            .navigationDestination(for: Int.self) { _ in full }
        }
    }

    private var full: some View {
        List {
            Section {
                Picker("Show", selection: .constant(0)) {
                    Text("Mine").tag(0)
                    Text("Review Requested").tag(1)
                }
                .pickerStyle(.segmented)
                .listRowBackground(Color.clear)
                .listRowInsets(EdgeInsets())
            }
            Section("Open") { ForEach(GH.mine.filter { $0.state != .merged }) { PullRow(pull: $0) } }
            Section("Merged this week") { ForEach(GH.mine.filter { $0.state == .merged }) { PullRow(pull: $0) } }
        }
        .listSectionSpacing(14)
        .navigationTitle("Pull Requests")
        .navigationBarTitleDisplayMode(.large)
    }
}

private struct Chip: View {
    var title: String
    var count: Int
    var on: Bool

    var body: some View {
        HStack(spacing: 5) {
            Text(title).lineLimit(1)
            Text("\(count)").monospacedDigit().foregroundStyle(on ? AnyShapeStyle(.white.opacity(0.8)) : AnyShapeStyle(.secondary))
        }
        .font(.subheadline.weight(.medium))
        .foregroundStyle(on ? AnyShapeStyle(.white) : AnyShapeStyle(.primary))
        .padding(.horizontal, 12).padding(.vertical, 6)
        .background(on ? AnyShapeStyle(.tint) : AnyShapeStyle(Color(.secondarySystemGroupedBackground)), in: .capsule)
        .fixedSize()
    }
}

private struct CompactPullRow: View {
    var pull: GHPull

    var body: some View {
        HStack(spacing: 10) {
            Image(systemName: pull.state.symbol).foregroundStyle(pull.state.color).frame(width: 20)
            VStack(alignment: .leading, spacing: 2) {
                Text(pull.title).lineLimit(1)
                HStack(spacing: 4) {
                    Text(verbatim: "\(pull.repo.split(separator: "/").last ?? "") #\(pull.number)")
                        .font(.caption.monospaced())
                    if pull.workspace == nil {
                        Text("· no workspace").font(.caption)
                    }
                }
                .foregroundStyle(.secondary).lineLimit(1)
            }
            Spacer(minLength: 4)
            GHCheckCounts(failing: pull.failing, pending: pull.pending, passed: 0)
        }
    }
}

private struct ComputerStub: View {
    var name: String
    var workspaces: [String]

    var body: some View {
        ForEach(workspaces, id: \.self) { workspace in
            HStack(spacing: 10) {
                Circle().fill(.green).frame(width: 10, height: 10)
                Text(workspace).font(.body.monospaced()).lineLimit(1)
                Spacer()
                if workspace == "phone-github-reads" {
                    Text(verbatim: "#1342").font(.caption.weight(.semibold).monospacedDigit()).foregroundStyle(.red)
                        .padding(.horizontal, 7).padding(.vertical, 2).background(Color.red.opacity(0.15), in: .capsule)
                }
            }
        }
    }
}

// MARK: C

struct PRListRepos: View {
    var state: Int

    var body: some View {
        GHTabs(selected: .workspaces) {
            Searching {
                List {
                    Section {
                        Picker("Show", selection: .constant(state)) {
                            Text("Mine · 4").tag(0)
                            Text("Review Requested · 3").tag(1)
                        }
                        .pickerStyle(.segmented)
                        .listRowBackground(Color.clear)
                        .listRowInsets(EdgeInsets())
                    }
                    let pulls = state == 1 ? GH.requested : GH.mine
                    ForEach(repos(pulls), id: \.self) { repo in
                        let inRepo = pulls.filter { $0.repo == repo }
                        Section {
                            ForEach(inRepo) { pull in RepoPullRow(pull: pull, showsAuthor: state == 1) }
                        } header: {
                            RepoHeader(repo: repo, hasWorkspace: inRepo.contains { $0.workspace != nil } || repo == GH.repo)
                        }
                    }
                }
                .listSectionSpacing(14)
            }
        }
    }

    private func repos(_ pulls: [GHPull]) -> [String] {
        pulls.reduce(into: [String]()) { if !$0.contains($1.repo) { $0.append($1.repo) } }
    }
}

private struct RepoHeader: View {
    var repo: String
    var hasWorkspace: Bool

    var body: some View {
        HStack(spacing: 8) {
            let name = String(repo.split(separator: "/").last ?? "")
            Text(name.prefix(1).uppercased()).font(.system(size: 11, weight: .bold, design: .rounded))
                .foregroundStyle(.white).frame(width: 20, height: 20)
                .background(hasWorkspace ? Color.green.gradient : Color.gray.gradient, in: .rect(cornerRadius: 6))
            Text(repo).font(.subheadline.weight(.semibold)).foregroundStyle(.primary).lineLimit(1)
            Spacer(minLength: 4)
            if !hasWorkspace {
                Text("No workspace").font(.caption2).foregroundStyle(.secondary)
                    .padding(.horizontal, 6).padding(.vertical, 2).background(.fill.tertiary, in: .capsule)
                    .fixedSize()
            }
        }
        .textCase(nil)
    }
}

private struct RepoPullRow: View {
    var pull: GHPull
    var showsAuthor: Bool

    var body: some View {
        HStack(alignment: .top, spacing: 10) {
            Image(systemName: pull.state.symbol).foregroundStyle(pull.state.color).frame(width: 20).padding(.top, 2)
            VStack(alignment: .leading, spacing: 4) {
                Text(pull.title).lineLimit(2)
                HStack(spacing: 6) {
                    Text(verbatim: "#\(pull.number)").font(.caption.monospaced())
                    if showsAuthor {
                        Text("by \(pull.author.login)").font(.caption)
                    } else if let computer = pull.workspace {
                        GHLabel(computer, systemImage: "laptopcomputer").font(.caption)
                    }
                    Text("· \(pull.updated)").font(.caption)
                }
                .foregroundStyle(.secondary).lineLimit(1)
            }
            Spacer(minLength: 4)
            VStack(alignment: .trailing, spacing: 4) {
                GHCheckCounts(failing: pull.failing, pending: pull.pending, passed: 0)
                if let review = pull.review, review != .required {
                    Image(systemName: review.symbol).font(.caption).foregroundStyle(review.color)
                }
            }
        }
    }
}

// MARK: D

struct PRListCards: View {
    var state: Int

    var body: some View {
        GHTabs(selected: .workspaces) {
            Searching {
                List {
                    Section {
                        ScrollView(.horizontal, showsIndicators: false) {
                            HStack(spacing: 10) {
                                ForEach(GH.requested) { ReviewCard(pull: $0) }
                            }
                            .scrollTargetLayout()
                        }
                        .scrollTargetBehavior(.viewAligned)
                        .contentMargins(.horizontal, 16, for: .scrollContent)
                        .listRowBackground(Color.clear)
                        .listRowInsets(EdgeInsets())
                    } header: {
                        Text("Review requested · 3")
                    }
                    Section("Mine · 4") {
                        ForEach(GH.mine) { CompactPullRow(pull: $0) }
                    }
                    if state == 1 {
                        Section("MacBook Pro") { ComputerStub(name: "MacBook Pro", workspaces: ["phone-github-reads"]) }
                    }
                }
                .listSectionSpacing(14)
            }
        }
    }
}

private struct ReviewCard: View {
    var pull: GHPull

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 6) {
                GHAvatar(person: pull.author, size: 22)
                Text(pull.author.login).font(.caption.weight(.semibold)).lineLimit(1)
                Spacer(minLength: 4)
                Text(pull.updated).font(.caption).foregroundStyle(.secondary)
            }
            Text(pull.title).font(.subheadline.weight(.semibold)).lineLimit(2)
                .frame(maxWidth: .infinity, minHeight: 40, alignment: .topLeading)
            Text(verbatim: "\(pull.repo) #\(pull.number)").font(.caption.monospaced()).foregroundStyle(.secondary)
                .lineLimit(1).truncationMode(.middle)
            HStack {
                GHCheckCounts(failing: pull.failing, pending: pull.pending, passed: pull.passed)
                Spacer(minLength: 4)
            }
            WhereLine(pull: pull)
        }
        .padding(14)
        .frame(width: 260)
        .background(Color(.secondarySystemGroupedBackground), in: .rect(cornerRadius: 22))
    }
}
