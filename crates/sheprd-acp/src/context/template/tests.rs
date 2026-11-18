use std::borrow::Cow;

use super::*;

fn fill(template: &str) -> String {
    render(template, |name| match name {
        "who" => Some(Cow::Borrowed("Ada")),
        "nested" => Some(Cow::Borrowed("{{ who }}")),
        _ => None,
    })
}

#[test]
fn placeholders_take_their_values_with_or_without_spaces() {
    assert_eq!(fill("Hi {{ who }}, {{who}}!"), "Hi Ada, Ada!");
    assert_eq!(fill("no placeholders"), "no placeholders");
}

#[test]
fn unknown_and_malformed_placeholders_stay_as_written() {
    assert_eq!(fill("{{ nobody }} and {{ who }}"), "{{ nobody }} and Ada");
    assert_eq!(
        fill("{{ Who }} {{ a-b }} {{}} {{ who"),
        "{{ Who }} {{ a-b }} {{}} {{ who"
    );
    assert_eq!(fill("{{ {{ who }}"), "{{ Ada");
    let long = format!("{{{{ {} }}}}", "a".repeat(65));
    assert_eq!(fill(&long), long);
}

#[test]
fn a_value_is_never_read_as_a_template() {
    assert_eq!(fill("{{ nested }}"), "{{ who }}");
}

#[test]
fn names_lists_each_placeholder_in_order() {
    assert_eq!(names("{{ a }} x {{b}} {{ Bad }} {{ a }}"), ["a", "b", "a"]);
}
