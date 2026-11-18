use super::*;

#[test]
fn a_plain_title_is_kept() {
    assert_eq!(
        title_from("Fix login redirect").as_deref(),
        Some("Fix login redirect")
    );
}

#[test]
fn quotes_labels_emphasis_and_a_final_period_are_dropped() {
    assert_eq!(
        title_from("\"Fix login redirect.\"").as_deref(),
        Some("Fix login redirect")
    );
    assert_eq!(
        title_from("**Title:** Relay setup").as_deref(),
        Some("Relay setup")
    );
    assert_eq!(title_from("“Tab titles”").as_deref(), Some("Tab titles"));
}

#[test]
fn a_long_answer_is_cut_on_a_word() {
    let title =
        title_from("Rework the phone session sheet so agents can name every chat tab").unwrap();
    assert!(title.chars().count() <= MAX_CHARS);
    assert_eq!(title, "Rework the phone session sheet so agents");
}

#[test]
fn one_long_word_is_cut_to_the_limit() {
    let title = title_from(&"x".repeat(100)).unwrap();
    assert_eq!(title.chars().count(), MAX_CHARS);
}

#[test]
fn an_empty_answer_has_no_title() {
    assert_eq!(title_from("  \"\" . "), None);
}
