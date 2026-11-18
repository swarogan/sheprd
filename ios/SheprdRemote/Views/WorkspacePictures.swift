import SheprdRemoteCore
import SwiftUI

/// Every picture in a workspace's chats, its tabs combined in one row that
/// scrolls left and right; nothing while there are none.
struct WorkspacePictures: View {
    let pictures: WorkspacePictureStore
    /// The workspace's chats, in the order its tab list shows them.
    let chats: [ChatItem]
    /// Shows a tab, as a tap on its row does.
    let open: (Route) -> Void

    /// Pictures kept per tab, newest first, so a long session full of
    /// screenshots neither crowds out the other tabs nor fills memory.
    private static let perTab = 40

    var body: some View {
        let shown = TabPictures.combine(chats.map { (tab: $0.chat.pane, pictures: pictures.byPane[$0.chat.pane] ?? []) }, perTab: Self.perTab)
            .map { picture in
                let chat = chats.first { $0.chat.pane == picture.tab }?.chat
                return GalleryPicture(picture, caption: chat?.title, agent: chat?.agent)
            }
        if !shown.isEmpty {
            Section {
                PictureStrip(pictures: shown) { picture in
                    guard let item = chats.first(where: { $0.chat.pane == picture.tab }) else { return }
                    open(.chat(item.key, title: item.chat.title))
                }
                    .listRowInsets(EdgeInsets())
            } header: {
                Text(shown.count == 1 ? "1 attachment" : "\(shown.count) attachments")
            }
        }
    }
}

/// The pictures of a workspace's chats, by pane: the phone's copies of the
/// chats first, then each chat as its computer has it.
@MainActor
@Observable
final class WorkspacePictureStore {
    private(set) var byPane: [String: [TranscriptPicture]] = [:]

    /// What a load reads; a change in it loads again.
    struct Load: Equatable {
        var panes: [String]
        var online: Bool
    }

    func load(_ load: Load, device: String, fleet: Fleet) async {
        let panes = load.panes
        let cache = fleet.cache
        let cached = await Task.detached(priority: .userInitiated) {
            panes.compactMap { pane in cache.chat(device, pane: pane).map { (pane, $0.transcript.pictures) } }
        }.value
        for (pane, pictures) in cached where byPane[pane] == nil { byPane[pane] = pictures }
        guard load.online else { return }
        for pane in panes {
            guard !Task.isCancelled else { return }
            if let pictures = try? await fleet.call(device, { client, token in
                try await client.chat(pane, bearer: token).transcript.pictures
            }) {
                byPane[pane] = pictures
            }
        }
    }
}
