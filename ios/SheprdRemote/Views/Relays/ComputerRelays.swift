import SheprdRemoteCore
import SwiftUI

/// A relay as its owner labelled it: the name over the address when it has
/// one, else the address, then its description and tags. The address keeps
/// its scheme, so `http://` and `https://` relays on one host differ.
struct RelayLabelView: View {
    let relay: RelayEndpoint
    let label: RelayLabel?
    /// Below the title, such as whether the relay answers.
    var status: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            if let name = label?.name {
                Text(name)
                address.font(.caption.monospaced()).foregroundStyle(.secondary)
            } else {
                address
            }
            if let status {
                Text(status).font(.caption).foregroundStyle(.secondary)
            }
            if let description = label?.description {
                Text(description).font(.caption).foregroundStyle(.secondary)
            }
            if let tags = label?.tags, !tags.isEmpty {
                RelayTags(tags: tags).padding(.top, 2)
            }
        }
    }

    private var address: some View {
        Text(verbatim: relay.description)
            .textSelection(.enabled)
            .lineLimit(1)
            .truncationMode(.middle)
    }
}

/// A relay's tags as small capsules, stacked when they do not fit a line.
private struct RelayTags: View {
    let tags: [String]

    var body: some View {
        ViewThatFits(in: .horizontal) {
            HStack(spacing: 4) { capsules }
            VStack(alignment: .leading, spacing: 4) { capsules }
        }
    }

    private var capsules: some View {
        // By position: nothing stops a tag from being given twice.
        ForEach(Array(tags.enumerated()), id: \.offset) { _, tag in
            Text(verbatim: tag)
                .font(.caption2.weight(.medium))
                .padding(.horizontal, 6).padding(.vertical, 2)
                .background(.tint.opacity(0.15), in: Capsule())
                .foregroundStyle(.tint)
                .lineLimit(1)
        }
    }
}
