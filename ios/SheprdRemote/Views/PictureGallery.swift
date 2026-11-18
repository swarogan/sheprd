import SheprdRemoteCore
import SwiftUI
import UIKit

extension EnvironmentValues {
    /// The open chat's pictures, which its full-screen viewer swipes through.
    @Entry var chatPictures: [GalleryPicture] = []
}

/// One picture the full-screen viewer can show, with what to say above it.
struct GalleryPicture: Identifiable, Hashable {
    let id: String
    let image: ChatImage
    /// The tab it came from, in a list of several tabs' pictures.
    var caption: String?
    /// The pane of that tab, which the viewer offers to open.
    var tab: String?
    /// The tab's agent, as Herdr names it, for its icon.
    var agent: String?

    /// A picture of one chat, named by its entry.
    init(_ picture: TranscriptPicture) {
        id = String(picture.id.value)
        image = picture.image
    }

    /// A picture of a workspace's combined list, named by its tab too, as
    /// two chats number their entries alike.
    init(_ picture: TabPicture<String>, caption: String?, agent: String?) {
        id = "\(picture.tab)/\(picture.id.entry.value)"
        image = picture.picture.image
        self.caption = caption
        tab = picture.tab
        self.agent = agent
    }
}

extension ChatImage {
    /// The picture, decoded off the main thread; scaled down to fit a
    /// square of `side` points when given, as a thumbnail needs no more.
    func decoded(fitting side: CGFloat? = nil) async -> UIImage? {
        let data = data
        return await Task.detached(priority: .userInitiated) {
            guard let bytes = Data(base64Encoded: data), let image = UIImage(data: bytes) else { return nil }
            guard let side else { return image }
            let scale = side * 3 / max(image.size.width, image.size.height, 1)
            guard scale < 1 else { return image }
            return await image.byPreparingThumbnail(ofSize: CGSize(width: image.size.width * scale, height: image.size.height * scale))
        }.value
    }
}

/// Pictures in a row that scrolls left and right; a tap shows one full
/// screen, where swiping goes through the rest and a button opens the tab
/// the picture shown came from.
struct PictureStrip: View {
    let pictures: [GalleryPicture]
    /// Opens a picture's tab, once the viewer has closed.
    var openTab: ((GalleryPicture) -> Void)?
    @State private var opened: GalleryPicture?
    /// The picture whose tab the viewer asked for as it closed.
    @State private var leavingFor: GalleryPicture?
    @Namespace private var zoom

    private static let side: CGFloat = 84

    var body: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            LazyHStack(alignment: .top, spacing: 10) {
                ForEach(pictures) { picture in
                    Button { opened = picture } label: {
                        VStack(alignment: .leading, spacing: 4) {
                            Thumbnail(image: picture.image, side: Self.side)
                                .matchedTransitionSource(id: picture.id, in: zoom)
                            if let caption = picture.caption {
                                Text(caption).font(.caption2).foregroundStyle(.secondary).lineLimit(1)
                                    .frame(width: Self.side, alignment: .leading)
                            }
                        }
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel(picture.caption.map { "Picture from \($0)" } ?? "Picture")
                    .accessibilityHint("Shows it full screen")
                }
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 12)
        }
        .scrollClipDisabled()
        // Navigating while the cover is still up would push behind it.
        .fullScreenCover(item: $opened, onDismiss: {
            guard let picture = leavingFor else { return }
            leavingFor = nil
            openTab?(picture)
        }) { picture in
            ImageViewer(pictures: pictures, selection: picture.id, openTab: openTab.map { _ in { leavingFor = $0 } })
                .navigationTransition(.zoom(sourceID: picture.id, in: zoom))
        }
    }
}

/// A square of a picture, decoded small and off the main thread.
private struct Thumbnail: View {
    let image: ChatImage
    let side: CGFloat
    @State private var decoded: UIImage?

    var body: some View {
        Group {
            if let decoded {
                Image(uiImage: decoded).resizable().scaledToFill()
            } else {
                Rectangle().fill(.fill.tertiary)
            }
        }
        .frame(width: side, height: side)
        .clipShape(.rect(cornerRadius: 12))
        .contentShape(.rect(cornerRadius: 12))
        .task(id: image) { decoded = await image.decoded(fitting: side) }
    }
}
