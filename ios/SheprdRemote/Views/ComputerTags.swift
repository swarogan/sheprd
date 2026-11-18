import SheprdRemoteCore
import SwiftUI

/// A tag on a capsule, in the color its name picks whatever its case.
struct TagPill: View {
    var tag: String
    /// Filled, as the filter's chosen tag is.
    var selected = false
    /// The mark after the name, for a pill that adds or removes the tag.
    var symbol: String?

    var body: some View {
        let color = RepositoryAvatar.tint(tag.lowercased())
        HStack(spacing: 3) {
            Text(tag).lineLimit(1)
            if let symbol { Image(systemName: symbol).imageScale(.small) }
        }
        .font(.caption.weight(.semibold))
        .foregroundStyle(selected ? .white : color)
        .padding(.horizontal, 8).padding(.vertical, 3)
        .background(selected ? color : color.opacity(0.15), in: .capsule)
    }
}

/// Pills in rows, wrapping to the width on offer.
struct PillFlow: Layout {
    var spacing: CGFloat = 6

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let frames = frames(subviews, width: proposal.width ?? .infinity)
        return CGSize(width: frames.map(\.maxX).max() ?? 0, height: frames.map(\.maxY).max() ?? 0)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        for (subview, frame) in zip(subviews, frames(subviews, width: bounds.width)) {
            subview.place(
                at: CGPoint(x: bounds.minX + frame.minX, y: bounds.minY + frame.minY),
                proposal: ProposedViewSize(frame.size)
            )
        }
    }

    private func frames(_ subviews: Subviews, width: CGFloat) -> [CGRect] {
        var origin = CGPoint.zero
        var rowHeight: CGFloat = 0
        return subviews.map { subview in
            var size = subview.sizeThatFits(.unspecified)
            size.width = min(size.width, width)
            if origin.x > 0, origin.x + size.width > width {
                origin = CGPoint(x: 0, y: origin.y + rowHeight + spacing)
                rowHeight = 0
            }
            defer {
                origin.x += size.width + spacing
                rowHeight = max(rowHeight, size.height)
            }
            return CGRect(origin: origin, size: size)
        }
    }
}

/// The tags in use and the computers, as a filter in the Workspaces
/// toolbar: a menu showing only the computers with a tag, or only one
/// computer, and all of them again. Computers appear once two could be
/// chosen. While one is chosen the button names it, a tag in its color, so
/// the list says why it is shorter without a row of its own above it. The
/// agent lists offer the same choices in their filter menu
/// (`AgentFilterMenu`).
struct TagFilterMenu: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        let computers = model.computers
        let tags = ComputerTags.all(in: computers)
        let active = ComputerTags.active(model.tagFilter.tag, among: computers)
        let device = ComputerTags.active(device: model.tagFilter.device, tag: active, among: computers)
        let hosts = computers.filter { $0.matches(tag: active) }
        let chosen = computers.first { $0.device == device }.map { model.displayName($0) }
        Menu {
            Picker("Show", selection: Binding(
                get: { active },
                set: { tag in withAnimation(.snappy) { model.tagFilter.choose(tag: tag, among: computers) } }
            )) {
                Label("All Computers", systemImage: "desktopcomputer").tag(String?.none)
                ForEach(tags, id: \.self) { tag in
                    Label(tag, systemImage: "tag").tag(Optional(tag))
                }
            }
            .pickerStyle(.inline)
            if hosts.count > 1 {
                Section("Computers") {
                    ForEach(hosts, id: \.device) { computer in
                        Toggle(model.displayName(computer), isOn: Binding(
                            get: { computer.device == device },
                            set: { on in withAnimation(.snappy) { model.tagFilter.device = on ? computer.device : nil } }
                        ))
                    }
                }
            }
        } label: {
            if let name = chosen ?? active {
                // Plain views, as a toolbar shows a `Label` as its icon alone.
                let color = chosen == nil ? RepositoryAvatar.tint(name.lowercased()) : Color.accentColor
                HStack(spacing: 5) {
                    Image(systemName: "line.3.horizontal.decrease")
                    Text(name).lineLimit(1)
                }
                .font(.subheadline.weight(.semibold))
                .foregroundStyle(color)
                .padding(.horizontal, 4)
            } else {
                Label("Filter Computers", systemImage: "line.3.horizontal.decrease")
            }
        }
        .accessibilityLabel(chosen.map { "Showing only \($0)" } ?? active.map { "Showing \($0) computers" } ?? "Filter Computers")
    }
}

/// A computer's tags on its page: a tap on one removes it, and a new one is
/// typed or picked from those in use.
struct ComputerTagsSection: View {
    @Environment(AppModel.self) private var model
    var computer: Computer
    @State private var draft = ""

    var body: some View {
        let suggestions = ComputerTags.suggestions(for: computer, among: model.computers)
        let full = computer.tags.count >= ComputerTags.maxPerComputer
        Section {
            if !computer.tags.isEmpty {
                PillFlow {
                    ForEach(computer.tags, id: \.self) { tag in
                        Button {
                            withAnimation(.snappy) { model.setTags(computer.tags.filter { $0 != tag }, for: computer) }
                        } label: {
                            TagPill(tag: tag, symbol: "xmark")
                        }
                        .buttonStyle(.plain)
                        .accessibilityLabel("Remove tag \(tag)")
                    }
                }
            }
            if !full {
                TextField("Add a tag", text: $draft)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .submitLabel(.done)
                    .onChange(of: draft) { draft = String(draft.prefix(ComputerTags.maxLength)) }
                    .onSubmit {
                        add(draft)
                        draft = ""
                    }
                if !suggestions.isEmpty {
                    PillFlow {
                        ForEach(suggestions, id: \.self) { tag in
                            Button {
                                add(tag)
                            } label: {
                                TagPill(tag: tag, symbol: "plus")
                            }
                            .buttonStyle(.plain)
                            .accessibilityLabel("Add tag \(tag)")
                        }
                    }
                }
            }
        } header: {
            Text("Tags")
        } footer: {
            Text("Tags sync through this computer to the phones and desktops paired with it. Filter by them at the top of Workspaces.")
        }
    }

    private func add(_ tag: String) {
        let known = ComputerTags.all(in: model.computers)
        withAnimation(.snappy) {
            model.setTags(ComputerTags.adding(tag, to: computer.tags, known: known), for: computer)
        }
    }
}
