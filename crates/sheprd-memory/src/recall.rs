//! Which words of a prompt are worth recalling entries by, and how many of
//! them an entry holds. FTS5 ranks the candidates; this keeps a long prompt
//! from matching every entry through a word such as "the".

use crate::Entry;

/// Words too common to say what a prompt is about.
const COMMON: &[&str] = &[
    "about", "after", "again", "all", "also", "and", "any", "are", "back", "been", "before", "but",
    "can", "could", "did", "does", "doing", "done", "for", "from", "get", "had", "has", "have",
    "here", "how", "into", "its", "just", "let", "like", "make", "more", "need", "not", "now",
    "our", "out", "please", "should", "some", "than", "that", "the", "their", "them", "then",
    "there", "these", "this", "use", "using", "want", "was", "were", "what", "when", "where",
    "which", "while", "who", "why", "will", "with", "would", "you", "your",
];

/// The most keywords one prompt contributes.
const MAX_KEYWORDS: usize = 24;

/// The distinct lowercase words of `prompt` of three letters or more that
/// are not [`COMMON`], in order.
pub(crate) fn keywords(prompt: &str) -> Vec<String> {
    let mut keywords: Vec<String> = Vec::new();
    for word in prompt.split(|c: char| !c.is_alphanumeric() && c != '_') {
        let word = word.to_lowercase();
        if word.chars().count() < 3 || COMMON.contains(&word.as_str()) || keywords.contains(&word) {
            continue;
        }
        keywords.push(word);
        if keywords.len() == MAX_KEYWORDS {
            break;
        }
    }
    keywords
}

/// How many of `keywords` begin a word of `entry`'s name, text, or details,
/// as FTS5 matched them.
pub(crate) fn matched(entry: &Entry, keywords: &[String]) -> usize {
    let mut words = Vec::new();
    for field in [
        entry.name.as_deref(),
        Some(entry.text.as_str()),
        entry.details.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        words.extend(
            field
                .split(|c: char| !c.is_alphanumeric() && c != '_')
                .filter(|word| !word.is_empty())
                .map(str::to_lowercase),
        );
    }
    keywords
        .iter()
        .filter(|keyword| words.iter().any(|word| word.starts_with(keyword.as_str())))
        .count()
}
