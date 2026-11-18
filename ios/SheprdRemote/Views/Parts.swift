import SheprdRemoteCore
import SwiftUI

/// An agent's mark: its logo, the one Sheprd and the web app draw
/// (`assets/icons/agent-*.svg`), on its brand's color, or on a plain tile
/// for marks without one.
struct AgentIcon: View {
    var agent: String?
    var size: CGFloat = 28

    private var look: (mark: String, tile: AnyShapeStyle, ink: Color) {
        let mark = AgentMark.named(agent)
        let brand: [String: Color] = [
            "claude": Color(red: 0xD9 / 255, green: 0x64 / 255, blue: 0x3A / 255),
            "codex": Color(red: 0x11 / 255, green: 0x11 / 255, blue: 0x11 / 255),
            "gemini": Color(red: 0x3B / 255, green: 0x7C / 255, blue: 0xF0 / 255),
            "copilot": Color(red: 0x6E / 255, green: 0x40 / 255, blue: 0xC9 / 255),
        ]
        guard let tile = brand[mark] else { return (mark, AnyShapeStyle(.fill.secondary), .primary) }
        return (mark, AnyShapeStyle(tile), .white)
    }

    var body: some View {
        let look = look
        Image("Agents/\(look.mark)")
            .resizable()
            .renderingMode(.template)
            .scaledToFit()
            .foregroundStyle(look.ink)
            .frame(width: size * 0.62, height: size * 0.62)
            .frame(width: size, height: size)
            .background(look.tile, in: .rect(cornerRadius: size * 0.28))
            .accessibilityLabel(agent ?? "Agent")
    }
}

/// Sheprd's working orbit: a faint ring with an arc turning on it once
/// every `period`. Every orbit reads the same clock, so all working
/// agents turn in step, as on the desktop. Held still under Reduce Motion.
struct WorkingOrbit: View {
    var color: Color = Home.AgentStatus.working.color
    var lineWidth: CGFloat = 1.8
    @Environment(\.accessibilityReduceMotion) private var still

    /// One turn, as the desktop's spinner takes.
    static let period = 0.9

    var body: some View {
        TimelineView(.animation(minimumInterval: 1 / 30, paused: still)) { context in
            let turn = still ? 0 : context.date.timeIntervalSinceReferenceDate
                .truncatingRemainder(dividingBy: Self.period) / Self.period
            ZStack {
                Circle().stroke(color.opacity(0.3), lineWidth: lineWidth)
                // A quarter of the way round, as `status-orbit.svg` draws it.
                Circle().trim(from: 0, to: 0.25)
                    .stroke(color, style: StrokeStyle(lineWidth: lineWidth, lineCap: .round))
                    .rotationEffect(.degrees(-90 + 360 * turn))
            }
        }
        .accessibilityLabel("Working")
    }
}

extension Home.AgentStatus {
    /// The color Sheprd's sidebar gives this status in its default
    /// palette: red for blocked, yellow for working, teal for done, green
    /// for idle.
    var color: Color {
        switch self {
        case .blocked: .red
        case .working: .yellow
        case .done: .teal
        case .idle: .green
        case .unknown: .secondary
        }
    }
}

/// An agent's status as a small badge on its icon's corner, so the title
/// line keeps its width: red when it needs you, its color with moving dots
/// while it works, a check once done, and nothing while idle.
struct StatusMark: View {
    var status: Home.AgentStatus
    var asking = false

    var body: some View {
        let shown: Home.AgentStatus = asking ? .blocked : status
        Group {
            switch shown {
            case .blocked: badge("exclamationmark", shown).accessibilityLabel("Needs you")
            case .working:
                badge("ellipsis", shown, animated: true).accessibilityLabel("Working")
            case .done: badge("checkmark", shown).accessibilityLabel("Done")
            case .idle, .unknown: EmptyView()
            }
        }
    }

    private func badge(_ symbol: String, _ status: Home.AgentStatus, animated: Bool = false) -> some View {
        Image(systemName: symbol)
            .font(.system(size: 8, weight: .black))
            .foregroundStyle(.white)
            .symbolEffect(.variableColor.iterative, isActive: animated)
            .frame(width: 15, height: 15)
            .background(status.color, in: .circle)
            // A ring of the row's own color parts the badge from the icon.
            .padding(2)
            .background(Color(uiColor: .secondarySystemGroupedBackground), in: .circle)
    }
}

extension View {
    /// `mark` on this icon's bottom corner, hanging just past its edge.
    func statusBadge(_ mark: StatusMark) -> some View {
        overlay(alignment: .bottomTrailing) { mark.offset(x: 6, y: 6) }
    }
}

struct ChatRowLabel: View {
    @AppStorage(RowLines.key) private var lines = RowLines.choices.lowerBound
    var item: ChatItem
    var showsComputer = true
    /// Off on the workspace's own screen, which names it already.
    var showsWorkspace = true
    /// Off on the workspace's own screen, which lists its pull request.
    /// When on, its pill says the state to VoiceOver, so the detail line does
    /// not repeat that it merged.
    var showsPullRequest = true

    var body: some View {
        // The timer and status share the title's line, so the detail
        // line below runs the full width.
        HStack(spacing: 12) {
            AgentIcon(agent: item.chat.agent)
                .statusBadge(StatusMark(status: item.chat.status, asking: item.question != nil))
            VStack(alignment: .leading, spacing: 2) {
                HStack(alignment: .firstTextBaseline, spacing: 6) {
                    Text(title).lineLimit(1)
                        .frame(maxWidth: .infinity, alignment: .leading)
                    if item.drafted {
                        Image(systemName: "pencil.circle.fill")
                            .font(.body).foregroundStyle(.tint)
                            .accessibilityLabel("Draft not sent")
                    }
                    if item.working, let started = item.chat.summary?.turnStartedMs {
                        Text(timerInterval: Date(timeIntervalSince1970: Double(started) / 1000)...Date.distantFuture, countsDown: false)
                            .font(.footnote.monospacedDigit()).foregroundStyle(.secondary)
                            .fixedSize()
                    } else if !item.needsYou, let date = item.lastAnswer {
                        LastAnswer(date: date)
                    }
                }
                HStack(alignment: .firstTextBaseline, spacing: 6) {
                    detail.font(.subheadline).foregroundStyle(.secondary).lineLimit(RowLines.clamped(lines))
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .accessibilityValue(showsWorkspace && !showsPullRequest && item.workspace.merged ? "Pull request merged" : "")
                    // Its workspace's pull request, colored by state, says at a
                    // glance whether the agent's work is ready or failing.
                    if showsPullRequest { WorkspacePullRequestBadge(workspace: item.workspace).fixedSize() }
                }
            }
        }
    }

    /// The chat's title, else its first prompt, else its agent's name, so a
    /// chat not yet titled still has a row that says what it is.
    private var title: String {
        let title = item.chat.title.trimmingCharacters(in: .whitespacesAndNewlines)
        if !title.isEmpty { return title }
        let prompt = item.chat.summary?.firstPrompt?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        if !prompt.isEmpty { return prompt }
        let agent = item.chat.agent?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        if awaitsFirstPrompt {
            return agent.isEmpty ? String(localized: "New chat") : String(localized: "New \(agent.capitalized) chat")
        }
        return agent.isEmpty ? String(localized: "Chat") : agent.capitalized
    }

    /// A linked chat nobody has written to yet: no title, no prompt, no
    /// turn, nothing said, and nothing asked. An older computer sends no
    /// first prompt, but a chat with a turn behind it fails this anyway.
    private var awaitsFirstPrompt: Bool {
        guard let summary = item.chat.summary, item.question == nil, !item.working else { return false }
        let blank = { (text: String?) in text?.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ?? true }
        return blank(item.chat.title) && blank(summary.firstPrompt) && blank(summary.lastText)
            && summary.turnStartedMs == nil && summary.lastTurnEndedMs == nil
    }

    /// The line under the title: a waiting question first, then the
    /// summary the person asked for while it still holds, then the latest
    /// reply.
    private var detail: Text {
        let summary = item.question == nil ? item.chat.summary?.currentRowSummary : nil
        switch summary {
        case let .ready(text, _)?:
            return Text("\(Image(systemName: "sparkles")) \(line(said: text))")
        case .pending?:
            return Text("\(Image(systemName: "sparkles")) \(line(said: String(localized: "Summarizing…")))")
        case let .failed(reason, _)?:
            return Text("\(Image(systemName: "exclamationmark.triangle")) \(line(said: String(localized: "No summary: \(reason)")))")
        case .other?, nil:
            if awaitsFirstPrompt { return Text(line(said: String(localized: "Waiting for your first prompt"))) }
            let said = (item.question?.title ?? item.chat.summary?.lastText)
                .map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
                .flatMap { $0.isEmpty ? nil : $0 }
            return Text(line(said: said))
        }
    }

    private func line(said: String?) -> AttributedString {
        let line = RowPlace.line([
            showsComputer ? AttributedString(item.computer.name) : nil,
            showsWorkspace ? RowPlace.name(of: item.workspace) : nil,
            said.map { AttributedString($0) },
        ])
        // Never a blank line: a chat with nothing said yet says so.
        guard line.characters.isEmpty else { return line }
        return AttributedString(item.working ? String(localized: "Working…") : String(localized: "No messages yet"))
    }
}

/// Where a row's agent runs, on the line under its title.
enum RowPlace {
    /// A workspace's name, in the color its pull request's badge has once
    /// merged, so a row whose badge is hidden still says it is done.
    static func name(of workspace: Home.Workspace) -> AttributedString {
        var name = AttributedString(workspace.label)
        if workspace.merged { name.foregroundColor = Home.PullRequest.Tone.merged.color }
        return name
    }

    static func line(_ parts: [AttributedString?]) -> AttributedString {
        parts.compactMap { $0 }.reduce(into: AttributedString()) { line, part in
            if !line.characters.isEmpty { line.append(AttributedString(" · ")) }
            line.append(part)
        }
    }
}

/// A terminal tab with an agent in it, such as `claude` run in a shell,
/// drawn like a chat row; it opens the terminal.
struct AgentTerminalRowLabel: View {
    var item: TerminalItem
    var showsComputer = true
    /// Off on the workspace's own screen, which shows the tab's name
    /// instead.
    var showsWorkspace = true
    /// Off on the workspace's own screen, which lists its pull request.
    /// When on, its pill says the state to VoiceOver, so the detail line does
    /// not repeat that it merged.
    var showsPullRequest = true

    var body: some View {
        HStack(spacing: 12) {
            AgentIcon(agent: item.agent?.name)
                .statusBadge(StatusMark(status: item.agent?.status ?? .unknown))
            VStack(alignment: .leading, spacing: 2) {
                HStack(alignment: .firstTextBaseline, spacing: 6) {
                    Text(item.title).lineLimit(1)
                        .frame(maxWidth: .infinity, alignment: .leading)
                    if !item.needsYou, !item.working, let date = item.changed { LastAnswer(date: date) }
                }
                HStack(alignment: .firstTextBaseline, spacing: 6) {
                    Text(place)
                        .font(.subheadline).foregroundStyle(.secondary).lineLimit(1)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .accessibilityValue(showsWorkspace && !showsPullRequest && item.workspace.merged ? "Pull request merged" : "")
                    if showsPullRequest { WorkspacePullRequestBadge(workspace: item.workspace).fixedSize() }
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    private var place: AttributedString {
        guard showsWorkspace else { return AttributedString(tabName) }
        return RowPlace.line([showsComputer ? AttributedString(item.computer.name) : nil, RowPlace.name(of: item.workspace)])
    }

    /// The tab's name, unless Sheprd only numbered it or it repeats the
    /// title: then the agent's name says more.
    private var tabName: String {
        let label = item.terminal.label
        let agent = item.agent?.name ?? "Agent"
        if label.allSatisfy(\.isNumber) || label == item.title { return agent }
        return "\(agent) · \(label)"
    }
}

/// How long ago a chat last answered, as a list shows it: "now", "5m",
/// "3h", a weekday within the week, then the date.
struct LastAnswer: View {
    let date: Date

    var body: some View {
        TimelineView(.everyMinute) { context in
            Text(Self.label(date, now: context.date))
                .font(.footnote.monospacedDigit()).foregroundStyle(.secondary)
                .fixedSize()
                .accessibilityLabel("Last answered \(date.formatted(.relative(presentation: .named)))")
        }
    }

    static func label(_ date: Date, now: Date) -> String {
        let seconds = now.timeIntervalSince(date)
        if seconds < 60 { return "now" }
        if seconds < 3600 { return "\(Int(seconds / 60))m" }
        if seconds < 86400 { return "\(Int(seconds / 3600))h" }
        if seconds < 6 * 86400 { return date.formatted(.dateTime.weekday(.abbreviated)) }
        return date.formatted(.dateTime.month(.abbreviated).day())
    }
}

/// A question with its options as buttons, answered in place.
struct AnswerCard: View {
    @Environment(AppModel.self) private var model
    var item: ChatItem
    var question: Permission
    @State private var sending = false
    @State private var problem: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 8) {
                AgentIcon(agent: item.chat.agent, size: 24)
                VStack(alignment: .leading, spacing: 0) {
                    Text(item.chat.title).font(.subheadline.weight(.semibold)).lineLimit(1)
                    Text("\(item.computer.name) · \(item.workspace.label)").font(.caption).foregroundStyle(.secondary)
                }
            }
            Text(question.title)
            // Permissions fit on one row; a question's answers get a row each.
            let compact = question.options.allSatisfy { $0.kind != .other } && question.options.count <= 4
            if compact {
                HStack {
                    ForEach(question.options) { option in
                        if option.kind == .allowOnce {
                            Button(option.name) { answer(option) }.buttonStyle(.glassProminent).tint(Home.AgentStatus.blocked.color).lineLimit(1)
                        } else {
                            Button(option.name) { answer(option) }.buttonStyle(.glass).lineLimit(1)
                        }
                    }
                }
            } else {
                ForEach(question.options) { option in
                    Button { answer(option) } label: { Text(option.name).frame(maxWidth: .infinity) }
                        .buttonStyle(.glass)
                }
            }
            if let problem { Text(problem).font(.footnote).foregroundStyle(.red) }
        }
        .disabled(sending)
        .padding(14)
        .background(.background.secondary, in: .rect(cornerRadius: 22))
        .overlay(RoundedRectangle(cornerRadius: 22).strokeBorder(Home.AgentStatus.blocked.color.opacity(0.35)))
    }

    private func answer(_ option: PermissionOption) {
        sending = true
        Task {
            guard await OwnerCheck.allows(option) else {
                sending = false
                return
            }
            problem = await model.fleet.send(.answer(permission: question.id, option: option.id), to: item.key)
            sending = false
        }
    }
}

extension Home.AgentStatus {
    var label: String {
        switch self {
        case .unknown: "Unknown"
        case .idle: "Idle"
        case .working: "Working"
        case .blocked: "Needs you"
        case .done: "Done"
        }
    }
}
