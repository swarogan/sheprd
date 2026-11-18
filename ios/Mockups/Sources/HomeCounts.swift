import SwiftUI

// Home with counts at the top and a way to fold every section, as the
// Workspaces tab folds its computers. State 0 is open, 1 all folded.
// A: a strip of count chips and Collapse All in the toolbar. B: a summary
// card with a stacked bar and the button. C: a 2×2 grid of count tiles
// that each fold their section. D: one quiet line of counts and the button.

private enum CountStatus: CaseIterable {
    case blocked, done, working, idle

    var title: String {
        switch self {
        case .blocked: "Needs you"
        case .done: "Done"
        case .working: "Working"
        case .idle: "Idle"
        }
    }

    var color: Color {
        switch self {
        case .blocked: .red
        case .done: .teal
        case .working: .orange
        case .idle: .gray
        }
    }

    var symbol: String {
        switch self {
        case .blocked: "exclamationmark.circle.fill"
        case .done: "checkmark.circle.fill"
        case .working: "ellipsis.circle.fill"
        case .idle: "moon.circle.fill"
        }
    }
}

private struct CountRow: Identifiable {
    let id = UUID()
    var agent: Agent
    var title: String
    var place: String
    var time: String
}

private struct Bucket: Identifiable {
    var status: CountStatus
    var count: Int
    var rows: [CountRow]
    var id: CountStatus { status }

    static let all: [Bucket] = [
        Bucket(status: .blocked, count: 2, rows: [
            CountRow(agent: .claude, title: "Relay picker question", place: "studio · proximity relays", time: "now"),
            CountRow(agent: .codex, title: "OC | Port workiq tests", place: "m5max · port: workiq", time: "1m"),
        ]),
        Bucket(status: .done, count: 23, rows: [
            CountRow(agent: .claude, title: "Closest relay selection", place: "studio · proximity relays", time: "now"),
            CountRow(agent: .codex, title: "OC | PR 172 code review", place: "m5max · port: atlas-wispr", time: "2m"),
            CountRow(agent: .codex, title: "OC | PR 173 code review", place: "m5max · port: workiq", time: "3m"),
            CountRow(agent: .codex, title: "OC | MS-HUB port tracking", place: "m5max · port: assistant", time: "4m"),
            CountRow(agent: .claude, title: "Sheprd relay propagation", place: "studio · worktree-green-valley", time: "4m"),
        ]),
        Bucket(status: .working, count: 4, rows: [
            CountRow(agent: .claude, title: "Home counts mockups", place: "m4max · herdr-gpui", time: "0:42"),
            CountRow(agent: .codex, title: "OC | Ingest backfill", place: "m5max · port: ingest", time: "3:10"),
        ]),
        Bucket(status: .idle, count: 6, rows: [
            CountRow(agent: .gemini, title: "Summarize relay PRs", place: "studio · herdr-gpui", time: "2h"),
        ]),
    ]

    static var total: Int { all.reduce(0) { $0 + $1.count } }
}

private func agentMark(_ agent: Agent) -> some View {
    Image(systemName: agent == .claude ? "asterisk" : agent == .codex ? "square" : agent.symbol)
        .font(.system(size: 15, weight: .heavy))
        .foregroundStyle(agent == .codex ? Color.primary : .white)
        .frame(width: 32, height: 32)
        .background(agent == .codex ? Color(.tertiarySystemFill) : agent.tint, in: .rect(cornerRadius: 9))
}

/// The Home tab around each variant: the tab bar, the toolbar, the search,
/// and the folding sections; `top` draws the counts above them.
private struct CountsScreen<Top: View>: View {
    var toolbarButton: Bool
    @State var folded: Set<CountStatus>
    @ViewBuilder var top: (Binding<Set<CountStatus>>) -> Top

    var body: some View {
        TabView {
            Tab("Home", systemImage: "house.fill") {
                NavigationStack { list }
            }
            .badge(Bucket.total)
            Tab("Workspaces", systemImage: "square.3.layers.3d") { Color.clear }
            Tab("Settings", systemImage: "gearshape.fill") { Color.clear }
        }
    }

    private var list: some View {
        List {
            top($folded)
            ForEach(Bucket.all) { group in
                Section {
                    if !folded.contains(group.status) {
                        ForEach(group.rows) { row($0, status: group.status) }
                    }
                } header: {
                    Button {
                        withAnimation(.snappy) {
                            if folded.contains(group.status) { folded.remove(group.status) } else { folded.insert(group.status) }
                        }
                    } label: {
                        HStack(spacing: 6) {
                            Image(systemName: "chevron.right").font(.caption.bold())
                                .rotationEffect(.degrees(folded.contains(group.status) ? 0 : 90))
                            Text(group.status.title)
                            Text("\(group.count)").foregroundStyle(.tertiary).monospacedDigit()
                            Spacer()
                        }
                        .font(.title3.weight(.semibold))
                        .foregroundStyle(.secondary)
                    }
                    .buttonStyle(.plain)
                    .textCase(nil)
                }
            }
        }
        .searchable(text: .constant(""), placement: .navigationBarDrawer(displayMode: .always),
                    prompt: "Workspaces, chats, terminals")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .topBarLeading) {
                Button("Sort", systemImage: "arrow.up.arrow.down") {}
            }
            if toolbarButton {
                ToolbarItem(placement: .topBarTrailing) {
                    let all = folded.count == CountStatus.allCases.count
                    Button(all ? "Expand All" : "Collapse All",
                           systemImage: all ? "rectangle.expand.vertical" : "rectangle.compress.vertical") {
                        withAnimation(.snappy) { folded = all ? [] : Set(CountStatus.allCases) }
                    }
                }
            }
            ToolbarItem(placement: .topBarTrailing) { Button("Talk", systemImage: "waveform") {} }
            ToolbarItem(placement: .topBarTrailing) { Button("New", systemImage: "plus") {} }
        }
    }

    private func row(_ row: CountRow, status: CountStatus) -> some View {
        NavigationLink {} label: {
            HStack(spacing: 12) {
                agentMark(row.agent)
                VStack(alignment: .leading, spacing: 2) {
                    HStack {
                        Text(row.title).lineLimit(1).frame(maxWidth: .infinity, alignment: .leading)
                        Text(row.time).font(.subheadline.monospacedDigit()).foregroundStyle(.secondary)
                    }
                    Text(row.place).font(.subheadline).foregroundStyle(.secondary).lineLimit(1)
                }
                Image(systemName: status.symbol).font(.title2).foregroundStyle(status.color)
            }
        }
    }
}

private func initial(_ state: Int) -> Set<CountStatus> { state == 1 ? Set(CountStatus.allCases) : [] }

// MARK: A

/// A strip of count chips above the list; Collapse All in the toolbar.
struct HomeCountsChips: View {
    var state: Int

    var body: some View {
        CountsScreen(toolbarButton: true, folded: initial(state)) { folded in
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 8) {
                    ForEach(Bucket.all) { group in
                        Button {
                            _ = withAnimation(.snappy) { folded.wrappedValue.remove(group.status) }
                        } label: {
                            HStack(spacing: 5) {
                                Image(systemName: group.status.symbol).foregroundStyle(group.status.color)
                                Text("\(group.count)").fontWeight(.semibold).monospacedDigit()
                            }
                            .font(.subheadline)
                            .padding(.horizontal, 12).padding(.vertical, 7)
                            .background(.background, in: .capsule)
                        }
                        .buttonStyle(.plain)
                    }
                }
                .padding(.horizontal, 20)
            }
            .listRowInsets(EdgeInsets())
            .listRowBackground(Color.clear)
        }
    }
}

// MARK: B

/// A summary card: the total, a bar split by status, and the button.
struct HomeCountsCard: View {
    var state: Int

    var body: some View {
        CountsScreen(toolbarButton: false, folded: initial(state)) { folded in
            Section {
                VStack(alignment: .leading, spacing: 10) {
                    HStack(alignment: .firstTextBaseline) {
                        Text("\(Bucket.total)").font(.largeTitle.bold()).monospacedDigit()
                        Text("agents").font(.title3).foregroundStyle(.secondary)
                        Spacer()
                        let all = folded.wrappedValue.count == CountStatus.allCases.count
                        Button(all ? "Expand All" : "Collapse All",
                               systemImage: all ? "rectangle.expand.vertical" : "rectangle.compress.vertical") {
                            withAnimation(.snappy) { folded.wrappedValue = all ? [] : Set(CountStatus.allCases) }
                        }
                        .font(.subheadline.weight(.medium))
                        .buttonStyle(.bordered).buttonBorderShape(.capsule)
                    }
                    GeometryReader { proxy in
                        HStack(spacing: 2) {
                            ForEach(Bucket.all) { group in
                                group.status.color
                                    .frame(width: max(4, (proxy.size.width - 6) * CGFloat(group.count) / CGFloat(Bucket.total)))
                            }
                        }
                        .clipShape(.capsule)
                    }
                    .frame(height: 8)
                    HStack(spacing: 14) {
                        ForEach(Bucket.all) { group in
                            HStack(spacing: 4) {
                                Circle().fill(group.status.color).frame(width: 8, height: 8)
                                Text("\(group.count)").fontWeight(.semibold).monospacedDigit()
                                Text(group.status.title).foregroundStyle(.secondary)
                            }
                            .lineLimit(1)
                        }
                    }
                    .font(.caption)
                }
                .padding(.vertical, 6)
            }
        }
    }
}

// MARK: C

/// Count tiles in a 2×2 grid; each tile folds or opens its section.
struct HomeCountsTiles: View {
    var state: Int

    var body: some View {
        CountsScreen(toolbarButton: true, folded: initial(state)) { folded in
            LazyVGrid(columns: [GridItem(.flexible(), spacing: 10), GridItem(.flexible(), spacing: 10)], spacing: 10) {
                ForEach(Bucket.all) { group in
                    let open = !folded.wrappedValue.contains(group.status)
                    Button {
                        withAnimation(.snappy) {
                            if open { folded.wrappedValue.insert(group.status) } else { folded.wrappedValue.remove(group.status) }
                        }
                    } label: {
                        VStack(alignment: .leading, spacing: 6) {
                            HStack {
                                Image(systemName: group.status.symbol).font(.title3).foregroundStyle(group.status.color)
                                Spacer()
                                Image(systemName: "chevron.right").font(.caption.bold()).foregroundStyle(.tertiary)
                                    .rotationEffect(.degrees(open ? 90 : 0))
                            }
                            Text("\(group.count)").font(.title.bold()).monospacedDigit()
                            Text(group.status.title).font(.subheadline).foregroundStyle(.secondary)
                        }
                        .padding(12)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .background(.background, in: .rect(cornerRadius: 18))
                        .overlay {
                            RoundedRectangle(cornerRadius: 18)
                                .strokeBorder(open ? group.status.color.opacity(0.6) : .clear, lineWidth: 1.5)
                        }
                    }
                    .buttonStyle(.plain)
                }
            }
            .listRowInsets(EdgeInsets(top: 0, leading: 20, bottom: 0, trailing: 20))
            .listRowBackground(Color.clear)
        }
    }
}

// MARK: D

/// One quiet line of counts with the button at its end.
struct HomeCountsLine: View {
    var state: Int

    var body: some View {
        CountsScreen(toolbarButton: false, folded: initial(state)) { folded in
            HStack {
                HStack(spacing: 4) {
                    Text("\(Bucket.total) agents").foregroundStyle(.secondary)
                    Text("·").foregroundStyle(.secondary)
                    Text("2 need you").foregroundStyle(.red).fontWeight(.semibold)
                }
                .font(.subheadline)
                Spacer()
                let all = folded.wrappedValue.count == CountStatus.allCases.count
                Button(all ? "Expand All" : "Collapse All") {
                    withAnimation(.snappy) { folded.wrappedValue = all ? [] : Set(CountStatus.allCases) }
                }
                .font(.subheadline.weight(.medium))
            }
            .listRowInsets(EdgeInsets(top: 0, leading: 20, bottom: 0, trailing: 20))
            .listRowBackground(Color.clear)
        }
    }
}
