import Foundation
import Testing
@testable import SheprdGitHub

/// GitHub comments' HTML as Markdown the native view draws.
struct CommentHTMLTests {
    @Test func plainMarkdownIsUntouched() {
        let text = "Looks good.\n\n- one\n- two"
        #expect(CommentHTML.markdown(text) == text)
    }

    @Test func aReviewBotsSummaryReadsAsMarkdown() {
        let html = """
        <!-- greptile_summary -->

        <h2><a href="https://app.greptile.com/api/retrigger?id=1&amp;sig=2"><picture><source media="(prefers-color-scheme: dark)" srcset="https://x/RetriggerDark.svg"><img alt="Retrigger" src="https://x/Retrigger.svg" align="right"></picture></a>Confidence Score: 4/5</h2>

        **[Critical impact]** The PR adds a path.

        <h2>Findings</h2>

        1. <img alt="P2" src="https://x/p2.svg" align="top">&nbsp;**Older summaries hide newer work** <a href="https://github.com/o/r/pull/1#discussion_r1">▶</a>
        """
        #expect(CommentHTML.markdown(html) == """
        ## [Retrigger](https://app.greptile.com/api/retrigger?id=1&sig=2)Confidence Score: 4/5

        **[Critical impact]** The PR adds a path.

        ## Findings

        1. `P2` **Older summaries hide newer work** [▶](https://github.com/o/r/pull/1#discussion_r1)
        """)
    }

    @Test func codeKeepsItsTagsAndEntities() {
        let text = """
        Use `<b>` and `&amp;` here, and Vec<T> in prose.

        ```html
        <b>bold</b> &amp;
        ```
        """
        #expect(CommentHTML.markdown(text) == text)
    }

    @Test func onlyWebLinksStayLinks() {
        #expect(CommentHTML.markdown(#"<a href="javascript:alert(1)">x</a> <a href="https://e.com">y</a>"#)
            == "x [y](https://e.com)")
    }

    @Test func detailsListsAndBreaks() {
        let html = "<details><summary>More</summary>line<br>next<ul><li>a</li><li>b</li></ul></details>"
        #expect(CommentHTML.markdown(html) == "**More**\n\nline  \nnext\n- a\n- b")
    }

    @Test func tablesBecomeMarkdownTables() {
        let html = "<table><tr><th>File</th><th>Note</th></tr><tr><td>a.rs</td><td>x | y</td></tr></table>"
        #expect(CommentHTML.markdown(html) == "| File | Note |\n| --- | --- |\n| a.rs | x \\| y |")
    }

    @Test func numericEntitiesDecode() {
        #expect(CommentHTML.markdown("a&#39;b &#x2014; c") == "a'b — c")
    }
}

/// Bounds on what a remote body may cost or produce.
struct CommentHTMLBoundsTests {
    @Test func aDecodedEntityCannotSmuggleADirectionOverride() {
        let body = PullRequestDetailAnswer.RemoteText.body("a&#x202E;b &#8238;c &#0;d", limit: 100)
        #expect(body == "ab c d")
    }

    @Test func manyUnclosedTagsStayCheap() {
        let text = String(repeating: "<a href=\"", count: 20_000)
        let start = Date()
        _ = CommentHTML.markdown(text)
        #expect(Date().timeIntervalSince(start) < 2)
    }
}
