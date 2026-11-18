import SwiftUI

// Home's agent rows with their workspace's pull request. A: the number in a
// colored pill at the end of the detail line (picked; what the app ships). B: the
// pill on the title line, before the status. C: a state icon and number
// inline in the detail line, no pill.

private enum AgentPRTone {
    case ready, waiting, attention, held, merged, unopened

    var color: Color {
        switch self {
        case .attention: .red
        case .held: .orange
        case .merged: .purple
        case .ready: .green
        case .waiting, .unopened: .yellow
        }
    }

    var symbol: String {
        switch self {
        case .attention: "xmark.circle.fill"
        case .held: "exclamationmark.triangle.fill"
        case .merged: "arrow.triangle.merge"
        case .ready: "checkmark.circle.fill"
        case .waiting: "clock.fill"
        case .unopened: "plus.circle.fill"
        }
    }
}

private struct AgentPRMark {
    var number: Int?
    var tone: AgentPRTone

    var text: String { number.map { "#\($0)" } ?? "+PR" }
}

private struct AgentRowData: Identifiable {
    var id: String { title }
    var agent: Agent
    var title: String
    var computer: String
    var workspace: String
    var said: String
    var time: String?
    var working = false
    var pr: AgentPRMark?
}

private let rows: [AgentRowData] = [
    AgentRowData(agent: .claude, title: "Fix relay reconnect backoff", computer: "Studio", workspace: "relay-backoff",
                 said: "Pushed; CI is running the Windows legs", working: true, pr: AgentPRMark(number: 1371, tone: .waiting)),
    AgentRowData(agent: .codex, title: "Add PR pills to agent rows", computer: "MacBook", workspace: "silver-harbor",
                 said: "Build passed, ready for review", time: "4m", pr: AgentPRMark(number: 1372, tone: .ready)),
    AgentRowData(agent: .claude, title: "Split the memory crate tests", computer: "Studio", workspace: "memory-tests",
                 said: "clippy failed on windows-native", time: "12m", pr: AgentPRMark(number: 1368, tone: .attention)),
    AgentRowData(agent: .gemini, title: "Tunnel rekey vectors", computer: "Linux box", workspace: "tunnel-rekey",
                 said: "Changes requested by a reviewer", time: "1h", pr: AgentPRMark(number: 1360, tone: .held)),
    AgentRowData(agent: .claude, title: "iOS module refactor", computer: "MacBook", workspace: "ios-modules",
                 said: "Merged into main", time: "2h", pr: AgentPRMark(number: 1365, tone: .merged)),
    AgentRowData(agent: .codex, title: "Explore usage panel layout", computer: "Studio", workspace: "usage-panel",
                 said: "3 commits, not pushed as a PR", time: "Mon", pr: AgentPRMark(number: nil, tone: .unopened)),
    AgentRowData(agent: .claude, title: "Answer a question about GPUI", computer: "MacBook", workspace: "main",
                 said: "No branch of its own", time: "Sun"),
]

struct AgentPRHome: View {
    var variant: String

    var body: some View {
        NavigationStack {
            List {
                Section("Working") {
                    ForEach(rows.filter(\.working)) { row in label(row) }
                }
                Section("Recent") {
                    ForEach(rows.filter { !$0.working }) { row in label(row) }
                }
            }
            .navigationTitle("Home")
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) { Image(systemName: "arrow.up.arrow.down") }
            }
        }
    }

    @ViewBuilder
    private func label(_ row: AgentRowData) -> some View {
        NavigationLink(value: row.title) {
            switch variant {
            case "B": AgentPRRowB(row: row)
            case "C": AgentPRRowC(row: row)
            default: AgentPRRowA(row: row)
            }
        }
    }
}

private struct AgentPRPill: View {
    var mark: AgentPRMark

    var body: some View {
        Text(verbatim: mark.text)
            .font(.caption.weight(.semibold).monospacedDigit())
            .foregroundStyle(mark.tone.color)
            .padding(.horizontal, 7).padding(.vertical, 2)
            .background(mark.tone.color.opacity(0.15), in: .capsule)
            .fixedSize()
    }
}

private struct AgentPRTrailing: View {
    var row: AgentRowData

    var body: some View {
        if row.working {
            Text("3:42").font(.footnote.monospacedDigit()).foregroundStyle(.secondary)
        } else if let time = row.time {
            Text(time).font(.footnote.monospacedDigit()).foregroundStyle(.secondary)
        }
    }
}

private func detail(_ row: AgentRowData) -> String {
    "\(row.computer) · \(row.workspace) · \(row.said)"
}

private struct AgentPRRowA: View {
    var row: AgentRowData

    var body: some View {
        HStack(spacing: 12) {
            AgentIcon(agent: row.agent)
            VStack(alignment: .leading, spacing: 2) {
                HStack(alignment: .firstTextBaseline, spacing: 6) {
                    Text(row.title).lineLimit(1).frame(maxWidth: .infinity, alignment: .leading)
                    AgentPRTrailing(row: row)
                }
                HStack(alignment: .firstTextBaseline, spacing: 6) {
                    Text(detail(row)).font(.subheadline).foregroundStyle(.secondary).lineLimit(1)
                        .frame(maxWidth: .infinity, alignment: .leading)
                    if let pr = row.pr { AgentPRPill(mark: pr) }
                }
            }
        }
    }
}

private struct AgentPRRowB: View {
    var row: AgentRowData

    var body: some View {
        HStack(spacing: 12) {
            AgentIcon(agent: row.agent)
            VStack(alignment: .leading, spacing: 2) {
                HStack(alignment: .firstTextBaseline, spacing: 6) {
                    Text(row.title).lineLimit(1).frame(maxWidth: .infinity, alignment: .leading)
                    if let pr = row.pr { AgentPRPill(mark: pr) }
                    AgentPRTrailing(row: row)
                }
                Text(detail(row)).font(.subheadline).foregroundStyle(.secondary).lineLimit(1)
            }
        }
    }
}

private struct AgentPRRowC: View {
    var row: AgentRowData

    var body: some View {
        HStack(spacing: 12) {
            AgentIcon(agent: row.agent)
            VStack(alignment: .leading, spacing: 2) {
                HStack(alignment: .firstTextBaseline, spacing: 6) {
                    Text(row.title).lineLimit(1).frame(maxWidth: .infinity, alignment: .leading)
                    AgentPRTrailing(row: row)
                }
                HStack(spacing: 4) {
                    if let pr = row.pr {
                        Image(systemName: pr.tone.symbol).foregroundStyle(pr.tone.color)
                        Text(verbatim: pr.text).monospacedDigit().foregroundStyle(pr.tone.color)
                            .fontWeight(.semibold).fixedSize()
                        Text(verbatim: "·").foregroundStyle(.secondary)
                    }
                    Text(detail(row)).foregroundStyle(.secondary).lineLimit(1)
                }
                .font(.subheadline)
            }
        }
    }
}
