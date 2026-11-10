//! Jira's Atlassian Document Format descriptions as Markdown.
use crate::jira::adf::{MAX_DEPTH, MAX_OUTPUT, to_markdown};
use serde_json::{Value, json};

fn doc(content: Vec<Value>) -> Value {
    json!({ "version": 1, "type": "doc", "content": content })
}

fn text(text: &str) -> Value {
    json!({ "type": "text", "text": text })
}

fn marked(value: &str, marks: Value) -> Value {
    json!({ "type": "text", "text": value, "marks": marks })
}

fn paragraph(content: Vec<Value>) -> Value {
    json!({ "type": "paragraph", "content": content })
}

fn says(content: Vec<Value>) -> String {
    to_markdown(&doc(content)).unwrap()
}

#[test]
fn paragraphs_and_headings_are_separated_by_blank_lines() {
    let heading = json!({ "type": "heading", "attrs": { "level": 2 }, "content": [text("Title")] });
    assert_eq!(
        says(vec![
            heading,
            paragraph(vec![text("Hello")]),
            paragraph(vec![text("Bye")])
        ]),
        "## Title\n\nHello\n\nBye"
    );
    // A level outside 1 to 6 is clamped, a missing or odd one is the first.
    let at = |level: Value| {
        says(vec![
            json!({ "type": "heading", "attrs": { "level": level }, "content": [text("T")] }),
        ])
    };
    assert_eq!(at(json!(9)), "###### T");
    assert_eq!(at(json!("x")), "# T");
}

#[test]
fn text_marks_become_markdown_around_the_words_not_their_spaces() {
    let line = paragraph(vec![
        text("a "),
        marked("bold", json!([{ "type": "strong" }])),
        text(" "),
        marked("it", json!([{ "type": "em" }])),
        text(" "),
        marked("x()", json!([{ "type": "code" }])),
        text(" "),
        marked("gone", json!([{ "type": "strike" }])),
        text(" "),
        marked(" pad ", json!([{ "type": "strong" }])),
        text(" "),
        marked(
            "site",
            json!([{ "type": "link", "attrs": { "href": "https://example.com/a b(c)" } }]),
        ),
        text(" "),
        marked(
            "both",
            json!([{ "type": "strong" }, { "type": "link", "attrs": { "href": "http://e.x" } }]),
        ),
        text(" "),
        marked("tick`s", json!([{ "type": "code" }])),
        text(" "),
        marked(
            "under",
            json!([{ "type": "underline" }, { "type": "textColor", "attrs": {} }]),
        ),
    ]);
    assert_eq!(
        says(vec![line]),
        "a **bold** *it* `x()` ~~gone~~  **pad**  [site](https://example.com/a%20b%28c%29) \
         [**both**](http://e.x) ``tick`s`` under"
    );
}

#[test]
fn a_link_to_a_script_or_a_file_keeps_its_text_and_loses_the_address() {
    for href in [
        "javascript:alert(1)",
        "file:///etc/passwd",
        "data:text/html,x",
    ] {
        let link = marked(
            "click",
            json!([{ "type": "link", "attrs": { "href": href } }]),
        );
        assert_eq!(says(vec![paragraph(vec![link])]), "click", "{href}");
    }
    let mail = marked(
        "me",
        json!([{ "type": "link", "attrs": { "href": "mailto:me@example.com" } }]),
    );
    assert_eq!(
        says(vec![paragraph(vec![mail])]),
        "[me](mailto:me@example.com)"
    );
}

fn item(content: Vec<Value>) -> Value {
    json!({ "type": "listItem", "content": content })
}

#[test]
fn lists_nest_and_number_from_their_start() {
    let nested = json!({ "type": "bulletList", "content": [
        item(vec![paragraph(vec![text("a")]), json!({ "type": "orderedList", "attrs": { "order": 3 },
            "content": [item(vec![paragraph(vec![text("x")])]), item(vec![paragraph(vec![text("y")])])] })]),
        item(vec![paragraph(vec![text("b")])]),
    ]});
    assert_eq!(says(vec![nested]), "- a\n  3. x\n  4. y\n- b");
    let numbered =
        json!({ "type": "orderedList", "content": [item(vec![paragraph(vec![text("one")])])] });
    assert_eq!(says(vec![numbered]), "1. one");
    // Two paragraphs in one item stay apart, the second under the marker.
    let two = json!({ "type": "bulletList", "content": [
        item(vec![paragraph(vec![text("p")]), paragraph(vec![text("q")])]),
    ]});
    assert_eq!(says(vec![two]), "- p\n\n  q");
}

#[test]
fn code_blocks_keep_their_language_and_outgrow_any_backticks_in_them() {
    let rust = json!({ "type": "codeBlock", "attrs": { "language": "rust" },
        "content": [text("fn main() {}\nlet x = 1;")] });
    assert_eq!(says(vec![rust]), "```rust\nfn main() {}\nlet x = 1;\n```");
    let bare = json!({ "type": "codeBlock", "content": [text("plain")] });
    assert_eq!(says(vec![bare]), "```\nplain\n```");
    let fenced = json!({ "type": "codeBlock", "content": [text("a ``` b")] });
    assert_eq!(says(vec![fenced]), "````\na ``` b\n````");
    // A language is a word, so it cannot carry anything onto the fence line.
    let odd =
        json!({ "type": "codeBlock", "attrs": { "language": "x y\n# h" }, "content": [text("c")] });
    assert_eq!(says(vec![odd]), "```\nc\n```");
}

#[test]
fn quotes_panels_and_rules() {
    let quote = json!({ "type": "blockquote", "content": [
        paragraph(vec![text("one")]), paragraph(vec![text("two")]),
    ]});
    assert_eq!(says(vec![quote]), "> one\n>\n> two");
    let panel = json!({ "type": "panel", "attrs": { "panelType": "info" },
        "content": [paragraph(vec![text("note")])] });
    assert_eq!(says(vec![panel]), "> note");
    assert_eq!(
        says(vec![
            paragraph(vec![text("a")]),
            json!({ "type": "rule" }),
            paragraph(vec![text("b")])
        ]),
        "a\n\n---\n\nb"
    );
}

#[test]
fn inline_nodes_say_what_they_show() {
    let line = paragraph(vec![
        text("one"),
        json!({ "type": "hardBreak" }),
        text("two "),
        json!({ "type": "mention", "attrs": { "id": "5b10", "text": "@Ada" } }),
        text(" "),
        json!({ "type": "emoji", "attrs": { "shortName": ":smile:", "text": "😄" } }),
        json!({ "type": "emoji", "attrs": { "shortName": ":custom:" } }),
        text(" "),
        json!({ "type": "inlineCard", "attrs": { "url": "https://acme.atlassian.net/browse/PROJ-1" } }),
        text(" "),
        json!({ "type": "status", "attrs": { "text": "DONE", "color": "green" } }),
        text(" "),
        json!({ "type": "date", "attrs": { "timestamp": "1790000000000" } }),
    ]);
    assert_eq!(
        says(vec![line]),
        "one  \ntwo @Ada 😄:custom: https://acme.atlassian.net/browse/PROJ-1 [DONE] 2026-09-21"
    );
    let card = json!({ "type": "blockCard", "attrs": { "url": "https://example.com/x" } });
    assert_eq!(says(vec![card]), "https://example.com/x");
}

#[test]
fn a_table_is_a_pipe_table_headed_by_its_first_row() {
    let cell = |kind: &str, value: &str| json!({ "type": kind, "content": [paragraph(vec![text(value)])] });
    let table = json!({ "type": "table", "content": [
        { "type": "tableRow", "content": [cell("tableHeader", "A"), cell("tableHeader", "B")] },
        { "type": "tableRow", "content": [cell("tableCell", "1"), cell("tableCell", "x|y")] },
        { "type": "tableRow", "content": [cell("tableCell", "short")] },
    ]});
    assert_eq!(
        says(vec![table]),
        "| A | B |\n| --- | --- |\n| 1 | x\\|y |\n| short |  |"
    );
}

#[test]
fn media_is_a_short_placeholder_and_never_fetched() {
    let single = json!({ "type": "mediaSingle", "attrs": { "layout": "center" }, "content": [
        { "type": "media", "attrs": { "id": "abc", "type": "file", "collection": "c" } },
    ]});
    assert_eq!(says(vec![single]), "[image]");
    let group = json!({ "type": "mediaGroup", "content": [
        { "type": "media", "attrs": { "type": "file", "alt": "logo [v2]" } },
        { "type": "media", "attrs": { "type": "external", "url": "https://tracker.example/p.png" } },
    ]});
    assert_eq!(says(vec![group]), "[image: logo  v2 ]\n[image]");
}

#[test]
fn a_node_it_does_not_know_keeps_its_text() {
    let expand = json!({ "type": "expand", "attrs": { "title": "More" }, "content": [
        paragraph(vec![text("inside")]),
        { "type": "nestedFuture", "content": [paragraph(vec![text("deeper")])] },
    ]});
    assert_eq!(says(vec![expand]), "inside\n\ndeeper");
    // Inline text under an unknown block reads as its paragraph.
    let task = json!({ "type": "taskList", "content": [
        { "type": "taskItem", "attrs": { "state": "TODO" }, "content": [text("ship it")] },
    ]});
    assert_eq!(says(vec![task]), "ship it");
    // An unknown inline node inside a paragraph, with children or a text of its own.
    let line = paragraph(vec![
        text("a "),
        json!({ "type": "futureInline", "content": [text("kept")] }),
        text(" "),
        json!({ "type": "futureLeaf", "text": "own" }),
    ]);
    assert_eq!(says(vec![line]), "a kept own");
}

#[test]
fn a_plain_string_is_its_own_text_and_anything_else_has_none() {
    assert_eq!(
        to_markdown(&json!("  Just text\nhere \n")).as_deref(),
        Some("Just text\nhere")
    );
    for value in [
        json!(null),
        json!(42),
        json!(true),
        json!([]),
        json!(""),
        json!("   "),
        json!({}),
        json!({ "type": "doc" }),
        json!({ "type": "doc", "content": "oops" }),
        json!({ "type": "doc", "content": [null, 1, "x", []] }),
        json!({ "type": "doc", "content": [{ "type": "paragraph" }] }),
        json!({ "type": "doc", "content": [{ "type": "paragraph", "content": [{ "type": "text" }] }] }),
        json!({ "type": "doc", "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": 5 }] }] }),
        json!({ "type": 7, "content": { "a": 1 } }),
    ] {
        assert_eq!(to_markdown(&value), None, "{value}");
    }
    // A malformed node beside a good one costs only itself.
    let mixed = doc(vec![json!(null), paragraph(vec![text("ok"), json!(3)])]);
    assert_eq!(to_markdown(&mixed).as_deref(), Some("ok"));
}

#[test]
fn nesting_deeper_than_the_bound_is_dropped_not_followed() {
    let nest = |levels: usize| {
        let mut node = paragraph(vec![text("bottom")]);
        for _ in 0..levels {
            node = json!({ "type": "blockquote", "content": [node] });
        }
        doc(vec![node])
    };
    // Well within the bound, the text and every quote mark are there.
    let shallow = to_markdown(&nest(5)).unwrap();
    assert_eq!(shallow, "> > > > > bottom");
    // Past it nothing is read, and no stack is used up: just no text.
    assert_eq!(to_markdown(&nest(MAX_DEPTH + 20)), None);
}

#[test]
fn the_output_stops_at_its_bound_on_a_character_boundary() {
    // Far more text than the bound, so the walk must stop early.
    let line = "é".repeat(100);
    let big = doc((0..5_000).map(|_| paragraph(vec![text(&line)])).collect());
    let out = to_markdown(&big).unwrap();
    assert!(out.len() <= MAX_OUTPUT, "{}", out.len());
    assert!(out.len() > MAX_OUTPUT - 1_000);
    // One paragraph past the bound by itself is cut as well.
    let one = doc(vec![paragraph(vec![text(&"é".repeat(MAX_OUTPUT))])]);
    let out = to_markdown(&one).unwrap();
    assert!(out.len() <= MAX_OUTPUT && out.chars().all(|c| c == 'é'));
}
