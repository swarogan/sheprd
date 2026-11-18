import Foundation

/// A picture in a chat, the user's or the agent's, as its entry shows it.
public struct TranscriptPicture: Hashable, Sendable, Identifiable {
    public var id: EntryId
    public var author: Author
    public var image: ChatImage

    public init(id: EntryId, author: Author, image: ChatImage) {
        self.id = id
        self.author = author
        self.image = image
    }
}

extension Transcript {
    /// The chat's pictures in the order it shows them, oldest first.
    public var pictures: [TranscriptPicture] {
        entries.compactMap {
            guard case let .image(author, image) = $0.kind else { return nil }
            return TranscriptPicture(id: $0.id, author: author, image: image)
        }
    }
}

/// One picture of a workspace's combined list, with the tab it came from.
public struct TabPicture<Tab: Hashable & Sendable>: Hashable, Sendable, Identifiable {
    public var tab: Tab
    public var picture: TranscriptPicture

    public var id: TabPictureId<Tab> { TabPictureId(tab: tab, entry: picture.id) }
}

public struct TabPictureId<Tab: Hashable & Sendable>: Hashable, Sendable {
    public var tab: Tab
    public var entry: EntryId
}

public enum TabPictures {
    /// The pictures of several tabs in one list: tab by tab in the order
    /// given, each tab's oldest first. A tab keeps its newest `perTab`, so
    /// one chat full of screenshots does not crowd out the others or hold
    /// every picture of a long session in memory.
    public static func combine<Tab: Hashable & Sendable>(
        _ tabs: some Sequence<(tab: Tab, pictures: [TranscriptPicture])>,
        perTab: Int
    ) -> [TabPicture<Tab>] {
        tabs.flatMap { tab, pictures in
            pictures.suffix(max(perTab, 0)).map { TabPicture(tab: tab, picture: $0) }
        }
    }
}
