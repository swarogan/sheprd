import Testing
@testable import SheprdRemoteCore

/// Home's status sections name their status, which their headers show as a
/// colored dot; the other orders group rows of mixed statuses.
struct AgentSectionStatusTests {
    private static func sections(_ sort: AgentSort, _ statuses: [Home.AgentStatus]) -> [String] {
        sort.sections(of: Array(statuses.enumerated())) { index, status in
            AgentSort.Facts(status: status, date: nil, title: "\(index)", group: "g", groupTitle: "g")
        }
        .map { "\($0.title) \($0.status?.rawValue ?? "none")" }
    }

    @Test func priorityTellsEachSectionItsStatus() {
        #expect(Self.sections(.priority, [.idle, .done, .working, .blocked])
            == ["Needs you blocked", "Done done", "Working working", "Idle idle"])
    }

    @Test func anUnknownStatusListsUnderIdle() {
        #expect(Self.sections(.priority, [.unknown]) == ["Idle idle"])
    }

    @Test func otherOrdersHaveNoStatus() {
        #expect(Self.sections(.recent, [.done, .idle]) == ["Recent none"])
        #expect(Self.sections(.workspace, [.done, .idle]) == ["g none"])
    }
}
