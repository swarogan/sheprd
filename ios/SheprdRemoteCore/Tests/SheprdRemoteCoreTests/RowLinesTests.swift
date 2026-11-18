@testable import SheprdRemoteCore
import Testing

/// The rows' line setting stays between one and three lines.
struct RowLinesTests {
    @Test func keepsAChoice() {
        for lines in RowLines.choices { #expect(RowLines.clamped(lines) == lines) }
    }

    @Test func boundsAnythingElse() {
        #expect(RowLines.clamped(0) == 1)
        #expect(RowLines.clamped(-4) == 1)
        #expect(RowLines.clamped(9) == 3)
    }
}
