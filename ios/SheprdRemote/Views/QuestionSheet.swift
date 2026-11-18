import SwiftUI
import SheprdRemoteCore

/// Questions A + B: a bottom sheet in thumb reach that scrolls, so a long
/// question is never cut short, and grows to full screen when scrolled up.
/// A question the agent asked with choices shows each option's
/// description, takes several picks when the agent allows them, and takes
/// the person's own words; other requests keep their plain options, the
/// "always" ones set apart.
struct QuestionSheet: View {
    @Environment(AppModel.self) private var model
    @Environment(\.dismiss) private var dismiss
    let key: PaneKey
    let permission: Permission
    @State private var chosen: Set<String> = []
    @State private var own = ""
    @FocusState private var typing: Bool

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 14) {
                if let header = permission.question?.header {
                    Text(header.uppercased()).font(.caption.weight(.semibold)).foregroundStyle(.secondary)
                }
                Text(permission.question?.text ?? permission.title)
                    .font(.title3.weight(.semibold))
                    .fixedSize(horizontal: false, vertical: true)
                    .textSelection(.enabled)
                if let question = permission.question {
                    choices(question)
                } else {
                    options
                }
            }
            .padding(24)
        }
        .scrollDismissesKeyboard(.interactively)
        .safeAreaInset(edge: .bottom) {
            if let question = permission.question { sendBar(question) }
        }
        .presentationDetents([.medium, .large])
        .presentationContentInteraction(.resizes)
        .presentationDragIndicator(.visible)
    }

    // MARK: A question with choices

    @ViewBuilder
    private func choices(_ question: AskedQuestion) -> some View {
        if question.multiple {
            Label("Choose any", systemImage: "checklist").font(.subheadline).foregroundStyle(.secondary)
        }
        ForEach(question.options) { option in
            let on = chosen.contains(option.label) && (question.multiple || ownWords.isEmpty)
            Button { pick(option.label, in: question) } label: {
                HStack(alignment: .top, spacing: 12) {
                    Image(systemName: mark(on: on, multiple: question.multiple)).font(.title3)
                        .foregroundStyle(on ? AnyShapeStyle(.tint) : AnyShapeStyle(.tertiary))
                    VStack(alignment: .leading, spacing: 3) {
                        Text(option.label).font(.body.weight(.medium))
                        if let description = option.description {
                            Text(description).font(.subheadline).foregroundStyle(.secondary)
                        }
                    }
                    .fixedSize(horizontal: false, vertical: true)
                    Spacer(minLength: 0)
                }
                .multilineTextAlignment(.leading)
                .padding(14)
                .frame(maxWidth: .infinity, alignment: .leading)
                .contentShape(.rect)
                .glassEffect(on ? .regular.tint(.accentColor.opacity(0.25)) : .regular, in: .rect(cornerRadius: 20))
            }
            .buttonStyle(.plain)
            .accessibilityAddTraits(on ? .isSelected : [])
        }
        TextField(question.multiple ? "Add your own answer" : "Or answer in your own words", text: $own, axis: .vertical)
            .lineLimit(1...5)
            .focused($typing)
            .padding(14)
            .glassEffect(.regular, in: .rect(cornerRadius: 20))
        if let chat = permission.options.first(where: { $0.kind == .rejectOnce }) {
            Button("Answer in the chat instead") { answer(chat) }
                .font(.subheadline)
                .frame(maxWidth: .infinity)
        }
    }

    private func sendBar(_ question: AskedQuestion) -> some View {
        let answers = question.answers(chosen: chosen, own: own)
        let title = question.multiple && answers.count > 1 ? "Send \(answers.count) answers" : "Send"
        return Button { send(answers) } label: { Text(title).frame(maxWidth: .infinity) }
            .buttonStyle(.glassProminent).controlSize(.large)
            .tint(Home.AgentStatus.blocked.color)
            .disabled(answers.isEmpty)
            .padding(.horizontal, 24).padding(.bottom, 8)
    }

    private var ownWords: String { own.trimmingCharacters(in: .whitespacesAndNewlines) }

    private func mark(on: Bool, multiple: Bool) -> String {
        multiple ? (on ? "checkmark.square.fill" : "square") : (on ? "largecircle.fill.circle" : "circle")
    }

    /// One choice replaces the other, and the typed answer, when the
    /// question takes only one.
    private func pick(_ label: String, in question: AskedQuestion) {
        if question.multiple {
            if chosen.remove(label) == nil { chosen.insert(label) }
        } else {
            chosen = [label]
            own = ""
            typing = false
        }
    }

    private func send(_ answers: [String]) {
        let fleet = model.fleet
        let command = ChatCommand.answerQuestion(permission: permission.id, answers: answers)
        Task { _ = await fleet.send(command, to: key) }
        dismiss()
    }

    // MARK: Plain options

    @ViewBuilder
    private var options: some View {
        let always = permission.options.filter { $0.kind == .allowAlways || $0.kind == .rejectAlways }
        ForEach(permission.options.filter { !always.contains($0) }) { option in
            if option.kind == .allowOnce {
                Button { answer(option) } label: { Text(option.name).frame(maxWidth: .infinity) }
                    .buttonStyle(.glassProminent).controlSize(.large).tint(Home.AgentStatus.blocked.color)
            } else {
                Button { answer(option) } label: { Text(option.name).frame(maxWidth: .infinity) }
                    .buttonStyle(.glass).controlSize(.large)
            }
        }
        if !always.isEmpty {
            Divider()
            ForEach(always) { option in
                Button(option.name) { answer(option) }.font(.subheadline)
            }
        }
    }

    /// The sheet leaves once the answer is allowed; a refused answer
    /// brings the question back, and the sync bar says why.
    private func answer(_ option: PermissionOption) {
        Task {
            guard await OwnerCheck.allows(option) else { return }
            let fleet = model.fleet
            Task { _ = await fleet.send(.answer(permission: permission.id, option: option.id), to: key) }
            dismiss()
        }
    }
}
