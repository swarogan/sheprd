import Foundation
@testable import SheprdRemoteCore
import Testing

/// A chat's pictures, and a workspace's pictures across its tabs, in the
/// order the viewer pages through them.
struct PicturesTests {
    private func image(_ name: String) -> ChatImage { ChatImage(data: name, mimeType: "image/png") }

    private func transcript(_ kinds: [EntryKind]) -> Transcript {
        var transcript = Transcript(agent: "claude")
        transcript.entries = kinds.enumerated().map { Entry(id: EntryId(UInt64($0.offset)), kind: $0.element) }
        return transcript
    }

    @Test func aChatKeepsOnlyItsPicturesInOrder() {
        let chat = transcript([
            .user(text: "look", files: 0),
            .image(author: .user, image: image("a")),
            .agent(text: "seen"),
            .image(author: .agent, image: image("b")),
        ])
        #expect(chat.pictures == [
            TranscriptPicture(id: 1, author: .user, image: image("a")),
            TranscriptPicture(id: 3, author: .agent, image: image("b")),
        ])
    }

    @Test func aChatWithoutPicturesHasNone() {
        #expect(transcript([.agent(text: "no pictures")]).pictures.isEmpty)
    }

    @Test func tabsCombineInTheirOrderOldestFirstWithin() {
        let first = [TranscriptPicture(id: 1, author: .user, image: image("a")), TranscriptPicture(id: 2, author: .agent, image: image("b"))]
        let second = [TranscriptPicture(id: 1, author: .agent, image: image("c"))]
        let combined = TabPictures.combine([(tab: "one", pictures: first), (tab: "two", pictures: second)], perTab: 10)
        #expect(combined.map(\.tab) == ["one", "one", "two"])
        #expect(combined.map(\.picture.image.data) == ["a", "b", "c"])
        // The same entry in two tabs is still two pictures.
        #expect(Set(combined.map(\.id)).count == combined.count)
    }

    @Test func eachTabKeepsItsNewestPictures() {
        let many = (0..<5).map { TranscriptPicture(id: EntryId(UInt64($0)), author: .agent, image: image("\($0)")) }
        let combined = TabPictures.combine([(tab: 1, pictures: many), (tab: 2, pictures: Array(many.prefix(1)))], perTab: 2)
        #expect(combined.map(\.picture.image.data) == ["3", "4", "0"])
        #expect(TabPictures.combine([(tab: 1, pictures: many)], perTab: -1).isEmpty)
    }
}
