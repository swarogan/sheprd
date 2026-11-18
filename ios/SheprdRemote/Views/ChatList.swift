import SheprdRemoteCore
import SwiftUI

/// Every chat across computers in the order the person chose, a question
/// answered in place. Agents run in terminal tabs join them, so Home lists
/// every agent the computers run. Home shows it.
///
/// Each section folds to its header as a computer does in the Workspaces
/// tab, a status section's header shows its colored dot, and a long list
/// gets an index on its edge with a letter per section and a dot per agent.
struct ChatList: View {
    @Environment(AppModel.self) private var model
    @AppStorage(HomeRow.sortKey) private var sort = AgentSort.priority
    var sections: [AgentSort.Section<HomeRow>]
    /// Where each section's header is, so a jump can clear the bars.
    @State private var headerJump = HeaderJump()
    /// What a swipe or long press asked to close, waiting for confirmation.
    @State private var closing: CloseTarget?

    /// What `CollapsedStore.home` keeps for `section`: a workspace and a
    /// status may share an id, so the order names it too.
    static func key(_ section: AgentSort.Section<HomeRow>, sort: AgentSort) -> String {
        "\(sort.rawValue)/\(section.id)"
    }

    /// Rows open at once before the list needs its index: about a
    /// screenful on a phone, as in Workspaces.
    private static let indexedFrom = 12

    /// Each section and the rows it lists, in order: what changes when a
    /// row moves, appears, or goes, but not when a row's text does.
    private var arrangement: [[String]] {
        sections.map { [$0.id] + $0.items.map(\.id) }
    }

    /// The id a section's header scrolls by.
    private static func headerID(_ key: String) -> String { "section/\(key)" }

    /// The sections as the edge index reads them: a folded one keeps only
    /// its letter.
    private var indexParts: [EdgeIndex.Part] {
        sections.map { section in
            let key = Self.key(section, sort: sort)
            let folded = model.collapsed.home.contains(key)
            return EdgeIndex.Part(
                target: Self.headerID(key),
                title: section.title,
                rows: folded ? [] : section.items.map { ($0.id, $0.status) }
            )
        }
    }

    var body: some View {
        let open = sections
            .filter { !model.collapsed.home.contains(Self.key($0, sort: sort)) }
            .reduce(0) { $0 + $1.items.count }
        ScrollViewReader { proxy in
            List {
                UnpairedCards()
                ForEach(sections) { section in
                    let key = Self.key(section, sort: sort)
                    let folded = model.collapsed.home.contains(key)
                    Section {
                        if !folded { ForEach(section.items) { row($0) } }
                    } header: {
                        SectionHeader(title: section.title, count: section.items.count, status: section.status, folded: folded) {
                            withAnimation(.snappy) { model.collapsed.home.toggle(key) }
                        }
                        .id(Self.headerID(key))
                        .background(HeaderMark(jump: headerJump, key: Self.headerID(key)))
                    }
                }
            }
            .overlay(alignment: .trailing) {
                if open >= Self.indexedFrom {
                    EdgeIndexStrip(parts: indexParts) { target in jump(to: target, proxy: proxy) }
                        .padding(.vertical, 24)
                        .padding(.trailing, 2)
                }
            }
            #if DEBUG
            .task { await followJumpArgument(proxy) }
            #endif
        }
        // Updates arrive from the computers unanimated; an agent going from
        // idle to working to done moves its row between sections, which the
        // list animates when the arrangement it shows changes.
        .animation(.snappy, value: arrangement)
        .refreshable { model.fleet.networkChanged() }
        .closeConfirmation($closing)
    }

    /// Scrolls to a header or row of the edge index, a header to just
    /// below the search field (`HeaderJump`), as a list's own jump leaves
    /// it behind the field. A section's first row shows its header too.
    private func jump(to target: String, proxy: ScrollViewProxy) {
        let header = sections.first { $0.items.first?.id == target }
            .map { Self.headerID(Self.key($0, sort: sort)) }
        headerJump.go(to: header ?? target, proxy: proxy)
    }

    #if DEBUG
    /// For the simulator loop, which cannot drag the edge index:
    /// `-homeJump N` scrolls to the bottom, then jumps to section N, or
    /// with `-homeJumpRow YES` to its first row, as its first dot does;
    /// `-homeJumpSweep YES` first crosses every entry from the bottom up,
    /// a frame apart, as a finger dragging up the index does.
    private func followJumpArgument(_ proxy: ScrollViewProxy) async {
        let index = UserDefaults.standard.integer(forKey: "homeJump")
        guard UserDefaults.standard.object(forKey: "homeJump") != nil else { return }
        try? await Task.sleep(for: .seconds(5))
        if let last = sections.last?.items.last { proxy.scrollTo(last.id, anchor: .bottom) }
        try? await Task.sleep(for: .seconds(2))
        guard sections.indices.contains(index) else { return }
        let section = sections[index]
        if UserDefaults.standard.bool(forKey: "homeJumpSweep") {
            for part in indexParts.reversed() {
                for row in part.rows.reversed() {
                    jump(to: row.id, proxy: proxy)
                    try? await Task.sleep(for: .milliseconds(16))
                }
                jump(to: part.target, proxy: proxy)
                try? await Task.sleep(for: .milliseconds(16))
            }
        }
        let row = UserDefaults.standard.bool(forKey: "homeJumpRow") ? section.items.first?.id : nil
        jump(to: row ?? Self.headerID(Self.key(section, sort: sort)), proxy: proxy)
    }
    #endif

    @ViewBuilder
    private func row(_ row: HomeRow) -> some View {
        // A section names the workspace when they are grouped by it.
        let grouped = sort == .workspace
        switch row {
        case let .chat(item):
            Group {
                if let question = item.question {
                    // The card draws its own background and border.
                    AnswerCard(item: item, question: question)
                        .listRowInsets(EdgeInsets(top: 4, leading: 0, bottom: 4, trailing: 0))
                        .listRowBackground(Color.clear)
                        .listRowSeparator(.hidden)
                } else {
                    NavigationLink(value: Route.chat(item.key, title: item.chat.title)) {
                        ChatRowLabel(item: item, showsComputer: !grouped, showsWorkspace: !grouped)
                    }
                }
            }
            .contextMenu {
                Button("Close Chat", systemImage: "xmark.circle", role: .destructive) { closing = .chat(item.key, title: item.chat.title) }
            }
            .closeSwipe(.chat(item.key, title: item.chat.title), closing: $closing)
            .summarizeSwipe(item.key)
        case let .agent(item):
            if let route = item.route {
                NavigationLink(value: route) { AgentTerminalRowLabel(item: item, showsComputer: !grouped, showsWorkspace: !grouped) }
                    .closeSwipe(item.closeTarget, closing: $closing)
            }
        }
    }
}

/// A status as a filled round icon, idle included, which `StatusMark`
/// leaves off a row's icon.
private struct StatusDot: View {
    var status: Home.AgentStatus

    var body: some View {
        let symbol = switch status {
        case .blocked: "exclamationmark.circle.fill"
        case .working: "ellipsis.circle.fill"
        case .done: "checkmark.circle.fill"
        case .idle, .unknown: "moon.circle.fill"
        }
        Image(systemName: symbol).foregroundStyle(status.color)
    }
}

/// A section's title and count, folding it on a tap.
private struct SectionHeader: View {
    var title: String
    var count: Int
    /// The status its rows share, shown as a colored dot.
    var status: Home.AgentStatus?
    var folded: Bool
    var toggle: () -> Void

    var body: some View {
        Button(action: toggle) {
            HStack(spacing: 6) {
                Image(systemName: "chevron.right").font(.caption.bold())
                    .rotationEffect(.degrees(folded ? 0 : 90))
                if let status { StatusDot(status: status) }
                Text(title).lineLimit(1)
                Text(count, format: .number).foregroundStyle(.secondary).monospacedDigit()
                Spacer(minLength: 0)
            }
            .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .accessibilityLabel("\(folded ? "Expand" : "Collapse") \(title), \(count)")
    }
}
