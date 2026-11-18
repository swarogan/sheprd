import SwiftUI

// Questions an agent asks, shown in full and answerable with several
// choices. State 0: one long single-choice question. State 1: a
// multi-select question with two picks made.
// A: the current sheet, scrolling, sized to its content, with option
//    descriptions and a pinned Send bar.
// B: a grouped list sheet with check rows and an "Other" answer.
// C: no sheet; a card in the transcript above the composer, with chips.
// D: a full-height page per question, "1 of 2", for several questions.

struct AskedOption: Identifiable {
    var id: String { label }
    var label: String
    var detail: String
    var recommended = false
}

struct Asked {
    var header: String
    var question: String
    var multiple: Bool
    var options: [AskedOption]
    var picked: Set<String>

    static let single = Asked(
        header: "Tab detail",
        question: "For plain terminal tabs on the iOS workspace screen, the phone only receives the tab name and its folder today; nothing about a prompt or command. What should the second line of those rows show, and should it fall back to the folder when a shell sits idle at its prompt?",
        multiple: false,
        options: [
            AskedOption(label: "Running command", detail: "What the foreground process is, e.g. cargo test; the folder when the shell is idle.", recommended: true),
            AskedOption(label: "Last command typed", detail: "Stays after the command ends, so an idle tab still says what it did."),
            AskedOption(label: "Short folder name", detail: "Today's line, shortened to the last two path parts."),
        ],
        picked: ["Running command"])

    static let multi = Asked(
        header: "Row details",
        question: "Which details should a terminal tab row show under its name? Pick any; rows grow to two lines only when something is chosen.",
        multiple: true,
        options: [
            AskedOption(label: "Running command", detail: "The foreground process, e.g. cargo test.", recommended: true),
            AskedOption(label: "Git branch", detail: "The branch of the tab's folder, when it is a checkout."),
            AskedOption(label: "Short folder name", detail: "The last two parts of the tab's folder."),
            AskedOption(label: "Exit status", detail: "A red mark when the last command failed."),
        ],
        picked: ["Running command", "Git branch"])

    static func sample(_ state: Int) -> Asked { state == 1 ? multi : single }
}

/// The chat behind every variant, so each sheet is seen in place.
private struct QuestionBackdrop<Sheet: View>: View {
    @ViewBuilder var sheet: Sheet

    var body: some View {
        ChatDocument()
            .sheet(isPresented: .constant(true)) { sheet }
    }
}

private struct PickMark: View {
    var multiple: Bool
    var on: Bool

    var body: some View {
        let name = multiple ? (on ? "checkmark.square.fill" : "square") : (on ? "largecircle.fill.circle" : "circle")
        Image(systemName: name).font(.title3)
            .foregroundStyle(on ? AnyShapeStyle(.tint) : AnyShapeStyle(.tertiary))
    }
}

private struct RecommendedBadge: View {
    var body: some View {
        Text("Recommended").font(.caption2.weight(.semibold))
            .padding(.horizontal, 6).padding(.vertical, 2)
            .background(.tint.opacity(0.15), in: .capsule)
            .foregroundStyle(.tint)
    }
}

private func toggle(_ label: String, in picked: inout Set<String>, multiple: Bool) {
    if multiple {
        if picked.contains(label) { picked.remove(label) } else { picked.insert(label) }
    } else {
        picked = [label]
    }
}

private func sendTitle(_ asked: Asked, _ picked: Set<String>) -> String {
    guard asked.multiple else { return "Send" }
    return picked.count == 1 ? "Send 1 answer" : "Send \(picked.count) answers"
}

// MARK: A

/// A: today's sheet, fixed: the question scrolls with the options instead
/// of being clipped, each option carries the description the agent wrote,
/// and the answer goes with a pinned Send.
struct QuestionScrollSheet: View {
    var state: Int

    var body: some View {
        QuestionBackdrop { Content(asked: .sample(state % 2), detent: state >= 2 ? .large : .medium) }
    }

    private struct Content: View {
        var asked: Asked
        var detent: PresentationDetent = .medium
        @State private var picked: Set<String> = []

        var body: some View {
            ScrollView {
                VStack(alignment: .leading, spacing: 12) {
                    Text(asked.header.uppercased()).font(.caption.weight(.semibold)).foregroundStyle(.secondary)
                    Text(asked.question).font(.title3.weight(.semibold))
                        .fixedSize(horizontal: false, vertical: true)
                    if asked.multiple {
                        Label("Choose any", systemImage: "checklist").font(.subheadline).foregroundStyle(.secondary)
                    }
                    ForEach(asked.options) { option in
                        let on = picked.contains(option.label)
                        Button { toggle(option.label, in: &picked, multiple: asked.multiple) } label: {
                            HStack(alignment: .top, spacing: 12) {
                                PickMark(multiple: asked.multiple, on: on)
                                VStack(alignment: .leading, spacing: 3) {
                                    HStack(spacing: 6) {
                                        Text(option.label).font(.body.weight(.medium))
                                        if option.recommended { RecommendedBadge() }
                                    }
                                    Text(option.detail).font(.subheadline).foregroundStyle(.secondary)
                                        .fixedSize(horizontal: false, vertical: true)
                                }
                                Spacer(minLength: 0)
                            }
                            .multilineTextAlignment(.leading)
                            .padding(14)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .glassEffect(on ? .regular.tint(.accentColor.opacity(0.25)) : .regular, in: .rect(cornerRadius: 20))
                        }
                        .buttonStyle(.plain)
                    }
                    TextField(asked.multiple ? "Add your own answer" : "Or answer in your own words", text: .constant(""), axis: .vertical)
                        .lineLimit(1...5)
                        .padding(14)
                        .glassEffect(.regular, in: .rect(cornerRadius: 20))
                    Button("Answer in the chat instead") {}.font(.subheadline)
                        .frame(maxWidth: .infinity).padding(.top, 4)
                }
                .padding(24)
            }
            .safeAreaInset(edge: .bottom) {
                Button { } label: { Text(sendTitle(asked, picked)).frame(maxWidth: .infinity) }
                    .buttonStyle(.glassProminent).controlSize(.large)
                    .disabled(picked.isEmpty)
                    .padding(.horizontal, 24).padding(.bottom, 8)
            }
            .presentationDetents([.medium, .large], selection: .constant(detent))
            .presentationContentInteraction(.resizes)
            .presentationDragIndicator(.visible)
            .onAppear { picked = asked.picked }
        }
    }
}

// MARK: B

/// B: a grouped list. The question is the section's header text, never
/// clipped; rows check like Settings; "Other" takes a typed answer.
struct QuestionListSheet: View {
    var state: Int

    var body: some View {
        QuestionBackdrop { Content(asked: .sample(state)) }
    }

    private struct Content: View {
        var asked: Asked
        @State private var picked: Set<String> = []
        @State private var other = ""

        var body: some View {
            NavigationStack {
                List {
                    Section {
                        Text(asked.question).font(.headline)
                            .fixedSize(horizontal: false, vertical: true)
                            .listRowBackground(Color.clear)
                    }
                    Section {
                        ForEach(asked.options) { option in
                            Button { toggle(option.label, in: &picked, multiple: asked.multiple) } label: {
                                HStack(spacing: 12) {
                                    VStack(alignment: .leading, spacing: 2) {
                                        HStack(spacing: 6) {
                                            Text(option.label).foregroundStyle(.primary)
                                            if option.recommended { RecommendedBadge() }
                                        }
                                        Text(option.detail).font(.caption).foregroundStyle(.secondary)
                                    }
                                    Spacer()
                                    if picked.contains(option.label) {
                                        Image(systemName: "checkmark").font(.body.weight(.semibold)).foregroundStyle(.tint)
                                    }
                                }
                                .contentShape(.rect)
                            }
                            .buttonStyle(.plain)
                        }
                        TextField("Other answer", text: $other, axis: .vertical).lineLimit(1...4)
                    } header: {
                        Text(asked.multiple ? "Choose any" : "Choose one")
                    } footer: {
                        Button("Answer in the chat instead") {}
                    }
                }
                .navigationTitle(asked.header)
                .navigationBarTitleDisplayMode(.inline)
                .toolbar {
                    ToolbarItem(placement: .cancellationAction) {
                        Button { } label: { Image(systemName: "xmark") }
                    }
                    ToolbarItem(placement: .confirmationAction) {
                        Button(asked.multiple ? "Send \(picked.count)" : "Send") {}
                            .buttonStyle(.glassProminent).disabled(picked.isEmpty)
                    }
                }
            }
            .presentationDetents([.large])
            .onAppear { picked = asked.picked }
        }
    }
}

// MARK: C

/// C: no sheet. The question sits in the transcript as the last message,
/// so it reads like the rest of the chat; chips wrap, and the composer's
/// send button sends the picks or a typed answer.
struct QuestionInlineCard: View {
    var state: Int
    @State private var picked: Set<String> = []

    var body: some View {
        let asked = Asked.sample(state)
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    Text(.init(SampleAnswer.intro)).foregroundStyle(.secondary)
                    VStack(alignment: .leading, spacing: 12) {
                        HStack(spacing: 6) {
                            Image(systemName: "questionmark.bubble.fill").foregroundStyle(.orange)
                            Text(asked.header).font(.subheadline.weight(.semibold))
                            Spacer()
                            Text(asked.multiple ? "Choose any" : "Choose one").font(.caption).foregroundStyle(.secondary)
                        }
                        Text(asked.question).font(.body)
                            .fixedSize(horizontal: false, vertical: true)
                        WrapLayout(spacing: 8) {
                            ForEach(asked.options) { option in
                                let on = picked.contains(option.label)
                                Button { toggle(option.label, in: &picked, multiple: asked.multiple) } label: {
                                    HStack(spacing: 6) {
                                        if on { Image(systemName: "checkmark").font(.caption.weight(.bold)) }
                                        Text(option.label + (option.recommended ? " ★" : ""))
                                    }
                                    .font(.subheadline.weight(.medium))
                                    .padding(.horizontal, 14).padding(.vertical, 9)
                                }
                                .buttonStyle(.plain)
                                .glassEffect(on ? .regular.tint(.accentColor.opacity(0.35)) : .regular, in: .capsule)
                            }
                        }
                        if let focus = asked.options.first(where: { picked.contains($0.label) }) {
                            Text("\(focus.label): \(focus.detail)").font(.caption).foregroundStyle(.secondary)
                        }
                    }
                    .padding(16)
                    .background(.orange.opacity(0.08), in: .rect(cornerRadius: 20))
                    .overlay(RoundedRectangle(cornerRadius: 20).strokeBorder(.orange.opacity(0.35)))
                }
                .padding(.horizontal, 16).padding(.bottom, 80)
            }
            .defaultScrollAnchor(.bottom)
            .navigationTitle("Tab list detail").navigationBarTitleDisplayMode(.inline)
            .safeAreaInset(edge: .bottom) {
                VStack(spacing: 8) {
                    Button { } label: {
                        Text(asked.multiple ? "Send \(picked.count) picks" : "Send “\(picked.first ?? "")”")
                            .frame(maxWidth: .infinity)
                    }
                    .buttonStyle(.glassProminent).controlSize(.large).padding(.horizontal, 16)
                    Composer(placeholder: "Or type an answer", working: false)
                }
            }
            .onAppear { picked = asked.picked }
        }
    }
}

/// Chips that wrap onto as many lines as they need.
private struct WrapLayout: Layout {
    var spacing: CGFloat

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let width = proposal.width ?? .infinity
        let rows = arrange(width: width, subviews: subviews)
        let height = rows.last.map { $0.y + $0.height } ?? 0
        return CGSize(width: proposal.width ?? rows.map(\.width).max() ?? 0, height: height)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        for row in arrange(width: bounds.width, subviews: subviews) {
            for (index, x) in row.items {
                subviews[index].place(at: CGPoint(x: bounds.minX + x, y: bounds.minY + row.y), proposal: .unspecified)
            }
        }
    }

    private struct Row { var items: [(Int, CGFloat)] = []; var y: CGFloat = 0; var width: CGFloat = 0; var height: CGFloat = 0 }

    private func arrange(width: CGFloat, subviews: Subviews) -> [Row] {
        var rows = [Row()]
        for (index, subview) in subviews.enumerated() {
            let size = subview.sizeThatFits(.unspecified)
            if rows[rows.count - 1].width + size.width > width, !rows[rows.count - 1].items.isEmpty {
                let last = rows[rows.count - 1]
                rows.append(Row(y: last.y + last.height + spacing))
            }
            let x = rows[rows.count - 1].items.isEmpty ? 0 : rows[rows.count - 1].width + spacing
            rows[rows.count - 1].items.append((index, x))
            rows[rows.count - 1].width = x + size.width
            rows[rows.count - 1].height = max(rows[rows.count - 1].height, size.height)
        }
        return rows
    }
}

// MARK: D

/// D: a page per question, for agents that ask several at once (today
/// those fall back to "Answer in the chat"). Back and Next move between
/// them; the last page sends every answer together.
struct QuestionPages: View {
    var state: Int

    var body: some View {
        QuestionBackdrop { Content(asked: .sample(state), page: state) }
    }

    private struct Content: View {
        var asked: Asked
        var page: Int
        @State private var picked: Set<String> = []

        var body: some View {
            VStack(alignment: .leading, spacing: 0) {
                HStack {
                    Button { } label: { Image(systemName: page == 0 ? "xmark" : "chevron.left").frame(width: 20, height: 20) }
                        .buttonStyle(.glass).buttonBorderShape(.circle)
                    Spacer()
                    HStack(spacing: 6) {
                        ForEach(0..<2) { index in
                            Capsule().fill(index <= page ? AnyShapeStyle(.tint) : AnyShapeStyle(.quaternary))
                                .frame(width: index == page ? 22 : 8, height: 8)
                        }
                    }
                    Spacer()
                    Text("\(page + 1) of 2").font(.subheadline.monospacedDigit()).foregroundStyle(.secondary)
                }
                .padding(.horizontal, 20).padding(.top, 20)
                ScrollView {
                    VStack(alignment: .leading, spacing: 14) {
                        Text(asked.header).font(.subheadline.weight(.semibold)).foregroundStyle(.tint)
                        Text(asked.question).font(.title2.weight(.bold))
                            .fixedSize(horizontal: false, vertical: true)
                        Text(asked.multiple ? "Choose any" : "Choose one").font(.subheadline).foregroundStyle(.secondary)
                        VStack(spacing: 0) {
                            ForEach(asked.options) { option in
                                let on = picked.contains(option.label)
                                Button { toggle(option.label, in: &picked, multiple: asked.multiple) } label: {
                                    HStack(alignment: .top, spacing: 12) {
                                        PickMark(multiple: asked.multiple, on: on)
                                        VStack(alignment: .leading, spacing: 2) {
                                            HStack(spacing: 6) {
                                                Text(option.label).font(.body.weight(.medium))
                                                if option.recommended { RecommendedBadge() }
                                            }
                                            Text(option.detail).font(.subheadline).foregroundStyle(.secondary)
                                                .fixedSize(horizontal: false, vertical: true)
                                        }
                                        Spacer(minLength: 0)
                                    }
                                    .multilineTextAlignment(.leading)
                                    .padding(.vertical, 12).padding(.horizontal, 14)
                                    .contentShape(.rect)
                                }
                                .buttonStyle(.plain)
                                if option.id != asked.options.last?.id { Divider().padding(.leading, 50) }
                            }
                        }
                        .background(.background.secondary, in: .rect(cornerRadius: 20))
                    }
                    .padding(20)
                }
                VStack(spacing: 10) {
                    Button { } label: {
                        Text(page == 0 ? "Next" : "Send both answers").frame(maxWidth: .infinity)
                    }
                    .buttonStyle(.glassProminent).controlSize(.large).disabled(picked.isEmpty)
                    Button("Answer in the chat instead") {}.font(.subheadline)
                }
                .padding(.horizontal, 20).padding(.bottom, 12)
            }
            .presentationDetents([.large])
            .onAppear { picked = asked.picked }
        }
    }
}
