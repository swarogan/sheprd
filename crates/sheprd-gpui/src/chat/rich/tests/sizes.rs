use super::*;

/// The paragraph's text, each run with the size it reads at.
fn runs(source: &str) -> Vec<(String, Option<Size>)> {
    let blocks = parse(source, &mut Visuals::default()).unwrap();
    let Block::Paragraph { content, .. } = &blocks[0] else {
        panic!("paragraph");
    };
    content
        .iter()
        .filter_map(|part| match part {
            Inline::Text(text, style) => Some((text.clone(), style.size)),
            _ => None,
        })
        .collect()
}

fn text(runs: &[(String, Option<Size>)]) -> String {
    runs.iter().map(|(text, _)| text.as_str()).collect()
}

#[test]
fn a_size_span_sets_its_text_and_leaves_out_its_marks() {
    let runs = runs("Tests pass and :huge[the PR is merged]. :small[CI took 4 min.]");
    assert_eq!(
        text(&runs),
        "Tests pass and the PR is merged. CI took 4 min."
    );
    assert!(runs.contains(&("the PR is merged".into(), Some(Size::Huge))));
    assert!(runs.contains(&("CI took 4 min.".into(), Some(Size::Small))));
    assert!(runs.contains(&("Tests pass and ".into(), None)));
}

#[test]
fn a_span_keeps_inline_styles_and_its_own_brackets() {
    let runs = runs(":big[a **bold** [note] word] after");
    assert_eq!(text(&runs), "a bold [note] word after");
    assert!(
        runs.iter()
            .any(|(text, size)| text == "bold" && *size == Some(Size::Big))
    );
    assert!(
        runs.iter()
            .any(|(text, size)| text.contains("[note] word") && *size == Some(Size::Big))
    );
    assert_eq!(runs.last().unwrap(), &(" after".to_owned(), None));
}

#[test]
fn unknown_names_code_and_addresses_stay_as_written() {
    for source in [
        ":large[x]",
        "`:big[x]`",
        "see http://host:big[x]",
        "word:big[x]",
    ] {
        let runs = runs(source);
        assert!(
            runs.iter().all(|(_, size)| size.is_none()),
            "{source}: {runs:?}"
        );
    }
}

#[test]
fn an_unclosed_span_ends_with_its_paragraph() {
    let blocks = parse(":giant[Still streaming\n\nNext", &mut Visuals::default()).unwrap();
    let sizes: Vec<_> = blocks
        .iter()
        .map(|block| match block {
            Block::Paragraph { content, .. } => match &content[0] {
                Inline::Text(_, style) => style.size,
                _ => None,
            },
            _ => None,
        })
        .collect();
    assert_eq!(sizes, [Some(Size::Giant), None]);
}

#[test]
fn spans_nest_to_a_bound() {
    let deep = ":big[".repeat(20) + "x" + &"]".repeat(20);
    let runs = runs(&deep);
    // The deepest openings beyond the bound stay text, but the text shows.
    assert!(text(&runs).contains('x'));
    assert!(runs.iter().any(|(_, size)| *size == Some(Size::Big)));
}
