@testable import SheprdRemoteCore
import Testing

/// The Home list's edge index: a letter per section and a dot per row.
struct EdgeIndexTests {
    private func part(_ title: String, _ count: Int, _ status: Home.AgentStatus = .idle) -> EdgeIndex.Part {
        EdgeIndex.Part(target: "s/\(title)", title: title, rows: (0..<count).map { ("\(title)/\($0)", status) })
    }

    @Test func aLetterThenADotPerRow() {
        let entries = EdgeIndex.entries([part("Done", 2, .done), part("idle", 1)], capacity: 50)
        #expect(entries.map(\.mark) == [.letter("D"), .dot(.done), .dot(.done), .letter("I"), .dot(.idle)])
        #expect(entries.map(\.target) == ["s/Done", "Done/0", "Done/1", "s/idle", "idle/0"])
    }

    @Test func aFoldedSectionKeepsItsLetter() {
        let entries = EdgeIndex.entries([part("Done", 0), part("Working", 2, .working)], capacity: 50)
        #expect(entries.first == EdgeIndex.Entry(target: "s/Done", mark: .letter("D")))
        #expect(entries.count == 4)
    }

    @Test func manyRowsAreThinnedToFit() {
        let parts = [part("Done", 20, .done), part("Working", 2, .working), part("Idle", 122)]
        let entries = EdgeIndex.entries(parts, capacity: 40)
        #expect(entries.count <= 40)
        let letters = entries.filter { if case .letter = $0.mark { true } else { false } }
        #expect(letters.count == 3)
        // Each section keeps at least one dot, and the biggest the most.
        let dots = { (status: Home.AgentStatus) in entries.filter { $0.mark == .dot(status) }.count }
        #expect(dots(.working) >= 1 && dots(.done) >= 1 && dots(.idle) > dots(.done))
        // Thinned dots still start at each section's first row.
        #expect(entries.contains { $0.target == "Idle/0" })
    }

    @Test func everySectionKeepsADotEvenWithoutRoom() {
        let entries = EdgeIndex.entries([part("Done", 5), part("Idle", 5)], capacity: 2)
        #expect(entries.map(\.target) == ["s/Done", "Done/0", "s/Idle", "Idle/0"])
    }
}
