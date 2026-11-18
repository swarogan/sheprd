//! `{{ name }}` placeholders in the prompt templates, as Moltis fills its
//! webhook templates: one pass, a value is never read as a template, and an
//! unknown name stays as written.

use std::borrow::Cow;

const OPEN: &str = "{{";
const CLOSE: &str = "}}";
/// The longest placeholder name; anything longer is plain text.
const MAX_NAME: usize = 64;

/// `template` with each known placeholder replaced by `value(name)`.
pub(crate) fn render<'a>(
    template: &str,
    mut value: impl FnMut(&str) -> Option<Cow<'a, str>>,
) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some((before, name, after)) = next(rest) {
        out.push_str(before);
        match value(name) {
            Some(value) => out.push_str(&value),
            None => out.push_str(&rest[before.len()..rest.len() - after.len()]),
        }
        rest = after;
    }
    out.push_str(rest);
    out
}

/// The placeholder names `template` uses, in order.
pub(crate) fn names(template: &str) -> Vec<&str> {
    let mut names = Vec::new();
    let mut rest = template;
    while let Some((_, name, after)) = next(rest) {
        names.push(name);
        rest = after;
    }
    names
}

/// The text before the next placeholder in `text`, its name, and the text
/// after it.
fn next(text: &str) -> Option<(&str, &str, &str)> {
    let mut from = 0;
    loop {
        let start = from + text[from..].find(OPEN)?;
        let inner = &text[start + OPEN.len()..];
        if let Some(end) = inner.find(CLOSE) {
            let name = inner[..end].trim();
            if is_name(name) {
                let after = &inner[end + CLOSE.len()..];
                return Some((&text[..start], name, after));
            }
        }
        from = start + OPEN.len();
    }
}

fn is_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_NAME
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

#[cfg(test)]
mod tests;
