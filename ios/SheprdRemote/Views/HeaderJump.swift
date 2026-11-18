import SwiftUI
import UIKit

/// Brings a list's section header to just below its bars.
///
/// A `List` scrolls to a header's id by lining the section's first row up
/// with the top of what the bars leave visible, which leaves the header
/// itself behind a search field, and SwiftUI offers no way to scroll a list
/// by a given amount. So each header carries a `HeaderMark`, an invisible
/// view whose window position says where the header is, and the jump moves
/// the list's own scroll view until that header's top meets the bars.
@MainActor
final class HeaderJump {
    private final class Mark {
        weak var view: MarkView?
    }

    private var marks: [String: Mark] = [:]
    /// The latest jump: a drag along the index starts one per entry it
    /// crosses, and only the one it ends on may still move the list.
    private var latest = 0
    /// Room left between the bars and the header's title.
    private static let gap: CGFloat = 8

    fileprivate func register(_ view: MarkView, for key: String) {
        // A list reuses a header's view for another header, so the view
        // says which header it shows now, and a stale entry is ignored.
        view.key = key
        let mark = marks[key] ?? Mark()
        mark.view = view
        marks[key] = mark
    }

    /// Brings the header marked `key` into view with `proxy`, then, once
    /// it is laid out, scrolls so it sits just below the bars. A later jump
    /// cancels this one's second step.
    func go(to key: String, proxy: ScrollViewProxy) {
        latest += 1
        let jump = latest
        proxy.scrollTo(key, anchor: .top)
        Task { @MainActor in
            // The header may take a few frames to be laid out where the
            // list scrolled; a later jump takes over.
            for _ in 0..<10 {
                await Task.yield()
                guard jump == latest else { return }
                if settle(key) { return }
                try? await Task.sleep(for: .milliseconds(16))
            }
        }
    }

    /// Scrolls so the header marked `key` sits just below the bars, or
    /// says it is not on screen yet.
    private func settle(_ key: String) -> Bool {
        guard let view = marks[key]?.view, view.key == key, view.window != nil,
              let scroll = sequence(first: view.superview, next: { $0?.superview }).lazy
                  .compactMap({ $0 as? UIScrollView }).first
        else { return false }
        scroll.layoutIfNeeded()
        let top = view.convert(view.bounds, to: scroll).minY
        let lowest = -scroll.adjustedContentInset.top
        let highest = max(lowest, scroll.contentSize.height + scroll.adjustedContentInset.bottom - scroll.bounds.height)
        let y = min(max(top - scroll.adjustedContentInset.top - Self.gap, lowest), highest)
        scroll.setContentOffset(CGPoint(x: scroll.contentOffset.x, y: y), animated: false)
        return true
    }
}

/// The invisible view a section header carries for `HeaderJump`, as wide
/// and tall as the header itself.
struct HeaderMark: UIViewRepresentable {
    let jump: HeaderJump
    let key: String

    func makeUIView(context: Context) -> MarkView {
        let view = MarkView()
        view.isUserInteractionEnabled = false
        view.backgroundColor = .clear
        return view
    }

    func updateUIView(_ view: MarkView, context: Context) {
        jump.register(view, for: key)
    }
}

/// A header's mark, knowing which header it shows now.
final class MarkView: UIView {
    var key: String?
}
