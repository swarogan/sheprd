import SwiftUI
import UIKit

extension View {
    /// Selects the whole text of an alert's field when it starts editing
    /// while `active`, so typing replaces it. For a rename alert's prefilled
    /// name: SwiftUI gives an alert's text field no selection, and modifiers
    /// on the field itself are dropped when the alert becomes a
    /// `UIAlertController`, so this watches for the field from the screen.
    func selectsAllOnEdit(while active: Bool) -> some View {
        modifier(SelectAllOnEdit(active: active))
    }
}

private struct SelectAllOnEdit: ViewModifier {
    let active: Bool
    /// Placed in this screen's window, so another window's fields, on an
    /// iPad with several, are left alone.
    @State private var anchor = UIView()

    func body(content: Content) -> some View {
        content
            .background(WindowAnchor(view: anchor).frame(width: 0, height: 0))
            .onReceive(NotificationCenter.default.publisher(for: UITextField.textDidBeginEditingNotification)) { note in
                guard active,
                      let field = note.object as? UITextField,
                      let scene = anchor.window?.windowScene,
                      field.window?.windowScene === scene,
                      field.isInAlert
                else { return }
                // After UIKit places the caret for this edit, or it wins.
                DispatchQueue.main.async { field.selectAll(nil) }
            }
    }
}

private struct WindowAnchor: UIViewRepresentable {
    let view: UIView

    func makeUIView(context: Context) -> UIView {
        view.isUserInteractionEnabled = false
        return view
    }

    func updateUIView(_ uiView: UIView, context: Context) {}
}

private extension UITextField {
    /// Whether the field belongs to an alert, not to the screen under it.
    var isInAlert: Bool {
        sequence(first: self as UIResponder, next: { $0.next }).contains { $0 is UIAlertController }
    }
}
