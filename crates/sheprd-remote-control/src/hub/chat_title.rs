//! A name for a chat: Claude, run once on its own as for a row summary
//! ([`super::row_summary`]), reads the end of the transcript and answers a
//! short title. Nothing is sent to the chat,
//! so its session and context are untouched; the phone renames the chat.

use super::row_summary;

/// The longest title kept, in characters; a phone's title bar shows about
/// this many beside its Back button.
pub(crate) const MAX_CHARS: usize = 40;

/// What the agent is asked; the transcript follows on its input. The limits
/// are spelled out because agents otherwise answer with a sentence.
const PROMPT: &str = "The input is the end of a conversation between a person and a coding \
agent. Reply with a short title for it, as a tab would show: 2 to 5 plain words, at most 40 \
characters, no quotes, punctuation, emoji, Markdown, or explanation.";

/// Blocking: the title for `input`, the excerpt of a transcript.
pub(crate) fn run(input: &str) -> Result<String, row_summary::Error> {
    let answer = row_summary::ask(PROMPT, input)?;
    title_from(&answer).ok_or(row_summary::Error::Empty)
}

/// The answer without the quotes, emphasis, label, or final period agents
/// add anyway, cut on a word to [`MAX_CHARS`].
pub(crate) fn title_from(answer: &str) -> Option<String> {
    const WRAPPING: &[char] = &['"', '\'', '`', '*', '_', '#', '“', '”', '‘', '’', ' ', '.'];
    let mut title = answer.trim_matches(WRAPPING);
    for label in ["Title:", "title:"] {
        if let Some(rest) = title.strip_prefix(label) {
            title = rest.trim_matches(WRAPPING);
        }
    }
    let mut words = title.split_whitespace();
    let mut kept: String = words.next()?.chars().take(MAX_CHARS).collect();
    for word in words {
        if kept.chars().count() + 1 + word.chars().count() > MAX_CHARS {
            break;
        }
        kept.push(' ');
        kept.push_str(word);
    }
    Some(kept)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;
