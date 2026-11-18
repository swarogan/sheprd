import SwiftUI
import UIKit

/// Shows a list's pull-to-refresh spinner while `active`, as if the person
/// had pulled, for a read they did not start but are waiting on.
///
/// SwiftUI shows the spinner of `refreshable` only during a pull, so this
/// invisible view, placed inside the list, finds the list's own scroll view
/// and drives its `UIRefreshControl`. Starting it programmatically sends no
/// action, so it reads nothing twice.
struct RefreshSpinner: UIViewRepresentable {
    var active: Bool

    /// Whether this view started the spinner, so it stops only its own
    /// and never one the person pulled.
    final class Coordinator {
        var started = false
    }

    func makeCoordinator() -> Coordinator { Coordinator() }

    func makeUIView(context: Context) -> UIView {
        let view = UIView()
        view.isUserInteractionEnabled = false
        return view
    }

    func updateUIView(_ view: UIView, context: Context) {
        let active = active
        let coordinator = context.coordinator
        // After this update is laid out, when the view sits in the list.
        Task { @MainActor in
            guard let scroll = sequence(first: view.superview, next: { $0?.superview }).lazy
                .compactMap({ $0 as? UIScrollView }).first,
                let control = scroll.refreshControl
            else { return }
            if active, !control.isRefreshing {
                let top = -scroll.adjustedContentInset.top
                coordinator.started = true
                control.beginRefreshing()
                // Revealed only from the top, as a pull would be; a list
                // scrolled down keeps its place.
                if scroll.contentOffset.y <= top + 1 {
                    scroll.setContentOffset(CGPoint(x: scroll.contentOffset.x, y: top - control.frame.height), animated: true)
                }
            } else if !active, coordinator.started {
                coordinator.started = false
                control.endRefreshing()
            }
        }
    }
}
