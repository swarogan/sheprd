import SheprdRemoteCore
import SwiftUI

/// Home's index on the list's trailing edge: each section's letter, then a
/// dot per agent in its status's color, so a list of three sections still
/// has a mark for every agent. A touch or a drag along it scrolls there.
///
/// The system index (`sectionIndexLabel`) lists sections only, and giving
/// each row a section of its own would break the grouped cards apart.
struct EdgeIndexStrip: View {
    var parts: [EdgeIndex.Part]
    var jump: (String) -> Void
    /// The entry under the finger, for the haptic as it changes.
    @State private var touched: String?

    private static let letterHeight: CGFloat = 15
    private static let dotHeight: CGFloat = 8
    private static let width: CGFloat = 18

    var body: some View {
        GeometryReader { space in
            let entries = EdgeIndex.entries(parts, capacity: capacity(height: space.size.height))
            VStack(spacing: 0) {
                ForEach(entries, id: \.target) { entry in
                    mark(entry.mark)
                }
            }
            .frame(width: Self.width)
            .contentShape(.rect)
            .gesture(
                DragGesture(minimumDistance: 0)
                    .onChanged { touch($0.location.y, entries) }
                    .onEnded { _ in touched = nil }
            )
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .trailing)
        }
        .frame(width: Self.width)
        .sensoryFeedback(.selection, trigger: touched) { _, new in new != nil }
        // VoiceOver moves by the section headers instead, as with the
        // system index.
        .accessibilityHidden(true)
    }

    /// How many entries fit `height`: every letter, then dots in the rest.
    private func capacity(height: CGFloat) -> Int {
        let letters = CGFloat(parts.count)
        let dots = (height - letters * Self.letterHeight) / Self.dotHeight
        return parts.count + max(Int(dots), 0)
    }

    @ViewBuilder
    private func mark(_ mark: EdgeIndex.Mark) -> some View {
        switch mark {
        case let .letter(letter):
            Text(letter)
                .font(.system(size: 11, weight: .semibold))
                .foregroundStyle(.tint)
                .frame(height: Self.letterHeight)
        case let .dot(status):
            Circle().fill(status.color).frame(width: 5, height: 5)
                .frame(height: Self.dotHeight)
        }
    }

    /// Scrolls to the entry at `y` along the strip.
    private func touch(_ y: CGFloat, _ entries: [EdgeIndex.Entry]) {
        var top: CGFloat = 0
        for entry in entries {
            let height = if case .letter = entry.mark { Self.letterHeight } else { Self.dotHeight }
            if y < top + height || entry == entries.last {
                guard touched != entry.target else { return }
                touched = entry.target
                jump(entry.target)
                return
            }
            top += height
        }
    }
}
