//! What memory refuses to keep. Entries are shown to later agents, some
//! inside their prompts, so a write that hides text, imitates Sheprd's own
//! markup, tells the reader to drop its instructions, or carries a
//! credential is refused rather than kept and repeated.
//!
//! This is a backstop, not the boundary: a phrase list cannot catch every
//! rewording. Readers treat entries as untrusted data instead: every line
//! quotes an entry's text (`Entry::line`), and the notes and skill that
//! show entries to agents say they are notes, never instructions.

/// Why a write was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Unsafe {
    #[error("Memory entries may not hold invisible or direction-changing characters")]
    Invisible,
    #[error("Memory entries may not hold prompt markup such as <system> or <sheprd>")]
    Markup,
    #[error("Memory entries may not tell agents to ignore their instructions")]
    Override,
    #[error("Memory entries may not hold secrets, keys, tokens, or passwords")]
    Secret,
}

/// Tags that Sheprd's notes or agents' prompts use.
const MARKUP: &[&str] = &[
    "<system",
    "</system",
    "<sheprd",
    "</sheprd",
    "<instructions",
    "</instructions",
    "<memory",
    "</memory",
    "<recalled",
    "</recalled",
    "<|im_start",
    "<|im_end",
    "[inst]",
    "[/inst]",
];

const OVERRIDES: &[&str] = &[
    "ignore previous instructions",
    "ignore all previous",
    "ignore the above",
    "ignore your instructions",
    "disregard previous",
    "disregard all previous",
    "disregard your instructions",
];

/// Prefixes of tokens that providers issue, each followed by at least
/// `min` more token characters.
const TOKENS: &[(&str, usize)] = &[
    ("sk-", 20),
    ("ghp_", 20),
    ("gho_", 20),
    ("ghs_", 20),
    ("ghu_", 20),
    ("github_pat_", 20),
    ("glpat-", 16),
    ("xoxb-", 10),
    ("xoxp-", 10),
    ("akia", 16),
];

/// Assignments whose value is a credential, as an environment file has them.
const ASSIGNED: &[&str] = &[
    "password",
    "passwd",
    "api_key",
    "apikey",
    "secret_key",
    "access_token",
    "auth_token",
    "client_secret",
];

/// Whether `text` may be kept.
pub(crate) fn check(text: &str) -> Result<(), Unsafe> {
    if text.chars().any(invisible) {
        return Err(Unsafe::Invisible);
    }
    let lower = text.to_lowercase();
    if MARKUP.iter().any(|tag| lower.contains(tag)) {
        return Err(Unsafe::Markup);
    }
    let words: Vec<&str> = lower.split_whitespace().collect();
    let spaced = words.join(" ");
    if OVERRIDES.iter().any(|phrase| spaced.contains(phrase)) {
        return Err(Unsafe::Override);
    }
    if lower.contains("-----begin") && lower.contains("private key") {
        return Err(Unsafe::Secret);
    }
    let next = words.iter().skip(1).map(Some).chain([None]);
    if words
        .iter()
        .zip(next)
        .any(|(word, next)| secret(word, next.copied()))
    {
        return Err(Unsafe::Secret);
    }
    Ok(())
}

fn invisible(c: char) -> bool {
    matches!(c,
        '\u{200B}'..='\u{200F}'
        | '\u{202A}'..='\u{202E}'
        | '\u{2060}'..='\u{2064}'
        | '\u{2066}'..='\u{2069}'
        | '\u{FEFF}')
        || (c.is_control() && !matches!(c, '\n' | '\t' | '\r'))
}

fn token_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '-')
}

/// Whether one lowercased word is, or assigns, a credential; `next` is the
/// word after it, the value of `api_key: VALUE`.
fn secret(word: &str, next: Option<&str>) -> bool {
    let word = word.trim_matches(|c: char| !token_char(c) && c != '=' && c != ':');
    // A token may stand alone or be the value of an assignment.
    let token = word.split(['=', ':']).any(|part| {
        let part = part.trim_matches(['"', '\'']);
        TOKENS.iter().any(|(prefix, min)| {
            part.strip_prefix(prefix)
                .is_some_and(|rest| rest.chars().take_while(|c| token_char(*c)).count() >= *min)
        })
    });
    let assigned = ASSIGNED.iter().any(|key| {
        word.strip_prefix(key)
            .and_then(|rest| rest.strip_prefix('=').or_else(|| rest.strip_prefix(':')))
            .map(|value| match (value, next) {
                ("", Some(next)) => next,
                _ => value,
            })
            .is_some_and(|value| value.trim_matches(['"', '\'']).chars().count() >= 4)
    });
    token || assigned
}
