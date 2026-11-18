import SheprdRemoteCore
import SwiftUI

/// What a row's swipe or long press may close: a chat's pane, which stops
/// its agent, or a terminal tab with whatever runs in it.
enum CloseTarget: Hashable, Identifiable {
    case chat(PaneKey, title: String)
    case tab(PaneKey, title: String)

    var id: Self { self }

    fileprivate var title: String {
        switch self {
        case let .chat(_, title), let .tab(_, title): title.isEmpty ? String(localized: "this tab") : title
        }
    }

    fileprivate var button: String {
        switch self {
        case .chat: String(localized: "Close Chat")
        case .tab: String(localized: "Close Tab")
        }
    }

    fileprivate var message: String {
        switch self {
        case .chat: String(localized: "Its pane closes in Sheprd and the agent stops.")
        case .tab: String(localized: "The tab closes in Sheprd with everything running in it.")
        }
    }
}

extension View {
    /// A red Close at the row's trailing edge. It only asks: the list's
    /// `closeConfirmation` closes once the person confirms.
    func closeSwipe(_ target: CloseTarget?, closing: Binding<CloseTarget?>) -> some View {
        swipeActions(edge: .trailing) {
            if let target {
                Button { closing.wrappedValue = target } label: { Label("Close", systemImage: "xmark") }
                    .tint(.red)
            }
        }
    }

    /// Asks before closing what a row offered, closes it, and says why when
    /// the computer refused.
    func closeConfirmation(_ closing: Binding<CloseTarget?>) -> some View {
        modifier(CloseConfirmation(closing: closing))
    }
}

private struct CloseConfirmation: ViewModifier {
    @Environment(AppModel.self) private var model
    @Binding var closing: CloseTarget?
    @State private var problem: String?

    func body(content: Content) -> some View {
        content
            .confirmationDialog(
                "Close \(closing?.title ?? "")?",
                isPresented: Binding(get: { closing != nil }, set: { if !$0 { closing = nil } }),
                titleVisibility: .visible,
                presenting: closing
            ) { target in
                Button(target.button, role: .destructive) { close(target) }
            } message: { target in
                Text(target.message)
            }
            .alert("It did not close", isPresented: Binding(get: { problem != nil }, set: { if !$0 { problem = nil } })) {
            } message: {
                Text(problem ?? "")
            }
    }

    private func close(_ target: CloseTarget) {
        Task {
            do {
                switch target {
                case let .chat(key, _): try await model.fleet.closeChatPane(key)
                case let .tab(key, _): try await model.fleet.closeTab(key)
                }
            } catch {
                problem = error.localizedDescription
            }
        }
    }
}
