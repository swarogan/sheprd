import SwiftUI

// A workspace's tab list. A chat's title is empty until its agent names
// it, so today the row's first line holds only the timer, pushed right.
// Now: today's row. A: the first prompt is the title until the agent names
// it. B: A with the time and status stacked in a column centered on the
// row. C: the prompt over two lines, the answer, and a status line.
// D: one centered line per tab, the answer only when it asks something.
// State 1 plays tabs coming and going: a new chat slides in, its first
// prompt becomes its title, and a closed tab slides out.

private enum TabState {
    case working(String), done(String), asking, idle(String), fresh
}

private struct TabSample: Identifiable {
    let id = UUID()
    var agent: Agent
    /// The agent's own name for the chat; empty until it sets one.
    var title: String
    var prompt: String?
    var answer: String?
    var state: TabState

    /// What the row names it by: the agent's title, else the first prompt.
    var name: String { title.isEmpty ? (prompt ?? "") : title }

    static let all: [TabSample] = [
        TabSample(agent: .claude, title: "", prompt: "Test the rename flow in the iOS simulator and fix what breaks",
                  answer: "I'll run it in a headless simulator again and watch the daemon log.", state: .working("0:49")),
        TabSample(agent: .claude, title: "", prompt: "Let me rename a workspace from the phone",
                  answer: "Renaming a workspace on iOS now shows a spinner until the computer confirms.", state: .done("now")),
        TabSample(agent: .codex, title: "Fix flaky pane test", prompt: "The pane_move test fails one run in ten",
                  answer: "Allow cargo test -p sheprd-gpui pane_move?", state: .asking),
        TabSample(agent: .gemini, title: "", prompt: "Summarize open PRs touching the relay",
                  answer: "Three PRs touch the relay: #1330, #1312 and #1298.", state: .idle("2h")),
        TabSample(agent: .claude, title: "", prompt: nil, answer: nil, state: .fresh),
    ]
}

private extension TabSample {
    /// The new chat, then the same chat once its first prompt is sent.
    static let fresh = all[all.count - 1]
    static let prompted: TabSample = {
        var tab = fresh
        tab.prompt = "Animate tabs as they appear and disappear"
        tab.answer = "Adding a transition to the tab rows in WorkspaceScreen."
        tab.state = .working("0:02")
        return tab
    }()
}

private extension TabState {
    var color: Color {
        switch self {
        case .working: .yellow
        case .done: .teal
        case .asking: .red
        case .idle, .fresh: .secondary
        }
    }

    var time: String? {
        switch self {
        case let .working(time), let .done(time), let .idle(time): time
        case .asking, .fresh: nil
        }
    }

    var word: String {
        switch self {
        case .working: "Working"
        case .done: "Done"
        case .asking: "Needs you"
        case .idle: "Idle"
        case .fresh: "New"
        }
    }

    @ViewBuilder var mark: some View {
        switch self {
        case .working: Image(systemName: "ellipsis").foregroundStyle(color)
        case .done: Image(systemName: "checkmark").font(.caption2.bold()).foregroundStyle(color)
        case .asking: Circle().fill(color).frame(width: 9, height: 9)
        case .idle, .fresh: EmptyView()
        }
    }

    @ViewBuilder var badge: some View {
        switch self {
        case .working: Image(systemName: "ellipsis.circle.fill").foregroundStyle(color)
        case .done: Image(systemName: "checkmark.circle.fill").foregroundStyle(color)
        case .asking: Image(systemName: "exclamationmark.circle.fill").foregroundStyle(color)
        case .idle, .fresh: EmptyView()
        }
    }
}

private struct ClaudeMark: View {
    var agent: Agent
    var size: CGFloat = 32

    var body: some View {
        Image(systemName: agent == .claude ? "asterisk" : agent.symbol)
            .font(.system(size: size * 0.5, weight: .bold))
            .foregroundStyle(.white)
            .frame(width: size, height: size)
            .background(agent.tint, in: .rect(cornerRadius: size * 0.28))
    }
}

/// The workspace screen around each variant.
private struct TabScreen<Row: View>: View {
    @ViewBuilder var row: (TabSample) -> Row
    @State private var tabs = TabSample.all.dropLast().map { $0 }


    var body: some View {
        NavigationStack {
            List {
                Section {
                    HStack(spacing: 12) {
                        Image(systemName: "arrow.triangle.branch").font(.system(size: 14, weight: .semibold))
                            .foregroundStyle(.tint)
                            .frame(width: 28, height: 28).background(.tint.opacity(0.15), in: .rect(cornerRadius: 8))
                        VStack(alignment: .leading, spacing: 2) {
                            Text("mobile-rename").font(.body.monospaced())
                            Text("studio · herdr-gpui").font(.subheadline).foregroundStyle(.secondary)
                        }
                    }
                }
                Section {
                    ForEach(tabs) { tab in
                        NavigationLink {} label: { row(tab) }
                    }
                    NavigationLink {} label: {
                        HStack(spacing: 12) {
                            Image(systemName: "apple.terminal").font(.system(size: 14, weight: .semibold))
                                .frame(width: 32, height: 32).background(.fill.secondary, in: .rect(cornerRadius: 9))
                            VStack(alignment: .leading, spacing: 2) {
                                Text("zsh")
                                Text("~/code/herdr-gpui").font(.subheadline).foregroundStyle(.secondary)
                            }
                        }
                    }
                } header: {
                    Text("\(tabs.count + 1) tabs").contentTransition(.numericText())
                }
            }
            .navigationTitle("mobile-rename")
            .navigationBarTitleDisplayMode(.inline)
            .task { if UserDefaults.standard.integer(forKey: "state") == 1 { await play() } else { tabs = TabSample.all } }
        }
    }

    /// Loops: a chat opens, gets its first prompt, then a tab closes and
    /// the list returns to where it started.
    private func play() async {
        let closed = tabs[2]
        while !Task.isCancelled {
            try? await Task.sleep(for: .seconds(1.2))
            withAnimation(.snappy) { tabs.append(TabSample.fresh) }
            try? await Task.sleep(for: .seconds(1.6))
            withAnimation(.snappy) { tabs[tabs.count - 1] = TabSample.prompted }
            try? await Task.sleep(for: .seconds(1.6))
            withAnimation(.snappy) { tabs.removeAll { $0.id == closed.id } }
            try? await Task.sleep(for: .seconds(1.6))
            withAnimation(.snappy) {
                tabs.removeAll { $0.id == TabSample.fresh.id }
                tabs.insert(closed, at: 2)
            }
        }
    }
}

// MARK: Now

struct TabListNow: View {
    var body: some View {
        TabScreen { tab in
            HStack(spacing: 12) {
                ClaudeMark(agent: tab.agent)
                VStack(alignment: .leading, spacing: 2) {
                    HStack(spacing: 6) {
                        Text(tab.title).lineLimit(1).frame(maxWidth: .infinity, alignment: .leading)
                        if let time = tab.state.time {
                            Text(time).font(.footnote.monospacedDigit()).foregroundStyle(.secondary)
                        }
                        tab.state.mark
                    }
                    Text(tab.answer ?? "")
                        .font(.subheadline).foregroundStyle(.secondary).lineLimit(1)
                }
            }
        }
    }
}

extension TabState: Equatable {}

// MARK: A

/// Today's row with the first prompt as its title, and a new chat saying
/// what it waits for.
struct TabListPromptTitle: View {
    var body: some View {
        TabScreen { tab in
            HStack(spacing: 12) {
                ClaudeMark(agent: tab.agent)
                VStack(alignment: .leading, spacing: 2) {
                    HStack(alignment: .firstTextBaseline, spacing: 6) {
                        Text(tab.state == .fresh ? "New \(tab.agent.name) chat" : tab.name).lineLimit(1)
                            .frame(maxWidth: .infinity, alignment: .leading)
                        if let time = tab.state.time {
                            Text(time).font(.footnote.monospacedDigit()).foregroundStyle(.secondary)
                        }
                        tab.state.mark
                    }
                    Text(tab.answer ?? "Waiting for your first prompt")
                        .font(.subheadline).foregroundStyle(tab.answer == nil ? .tertiary : .secondary).lineLimit(1)
                }
            }
        }
    }
}

// MARK: B

/// A's text, with the time over the status in a column centered on the row.
struct TabListCentered: View {
    var body: some View {
        TabScreen { tab in
            HStack(spacing: 12) {
                ClaudeMark(agent: tab.agent)
                VStack(alignment: .leading, spacing: 2) {
                    Text(tab.state == .fresh ? "New \(tab.agent.name) chat" : tab.name)
                        .font(.body.weight(.medium)).lineLimit(1)
                    Text(tab.answer ?? "Waiting for your first prompt")
                        .font(.subheadline).foregroundStyle(tab.answer == nil ? .tertiary : .secondary).lineLimit(1)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                VStack(spacing: 3) {
                    tab.state.badge.imageScale(.medium)
                    if let time = tab.state.time {
                        Text(time).font(.caption2.monospacedDigit()).foregroundStyle(.secondary)
                    }
                }
                .frame(minWidth: 30)
            }
            .padding(.vertical, 2)
        }
    }
}

// MARK: C

/// The prompt over up to two lines, the answer, and a status line.
struct TabListThreeLines: View {
    var body: some View {
        TabScreen { tab in
            HStack(alignment: .top, spacing: 12) {
                ClaudeMark(agent: tab.agent, size: 36)
                VStack(alignment: .leading, spacing: 3) {
                    Text(tab.state == .fresh ? "New \(tab.agent.name) chat" : tab.name)
                        .font(.body.weight(.semibold)).lineLimit(2)
                    if let answer = tab.answer {
                        Text(answer).font(.subheadline).foregroundStyle(.secondary).lineLimit(1)
                    }
                    HStack(spacing: 5) {
                        Circle().fill(tab.state.color).frame(width: 7, height: 7)
                        Text(tab.state == .fresh ? "Waiting for your first prompt" : tab.state.word)
                        if let time = tab.state.time { Text("· \(time)").monospacedDigit() }
                    }
                    .font(.caption.weight(.medium)).foregroundStyle(tab.state == .fresh ? .tertiary : .secondary)
                    .padding(.top, 1)
                }
            }
            .padding(.vertical, 4)
        }
    }
}

// MARK: D

/// One line per tab, centered on the icon; the answer shows only when the
/// agent asks something.
struct TabListOneLine: View {
    var body: some View {
        TabScreen { tab in
            HStack(spacing: 12) {
                ClaudeMark(agent: tab.agent, size: 28)
                VStack(alignment: .leading, spacing: 2) {
                    Text(tab.state == .fresh ? "New \(tab.agent.name) chat" : tab.name).lineLimit(1)
                        .foregroundStyle(tab.state == .fresh ? .secondary : .primary)
                    if tab.state == .asking, let answer = tab.answer {
                        Text(answer).font(.subheadline).foregroundStyle(.red).lineLimit(1)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                if tab.state != .fresh {
                    HStack(spacing: 4) {
                        if tab.state != .asking { tab.state.mark }
                        Text(tab.state.time ?? tab.state.word).monospacedDigit()
                    }
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(tab.state == .asking ? .white : .secondary)
                    .padding(.horizontal, 8).padding(.vertical, 4)
                    .background(tab.state == .asking ? AnyShapeStyle(Color.red) : AnyShapeStyle(.fill.tertiary), in: .capsule)
                }
            }
        }
    }
}
