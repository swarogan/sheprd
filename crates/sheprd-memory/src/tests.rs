#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::*;

mod checkout;
mod recall;
mod safety;
mod search;
mod store;

const NOW: i64 = 1_700_000_000;

fn fact(text: &str) -> New<'_> {
    New {
        text,
        ..New::default()
    }
}

fn texts(entries: &[Entry]) -> Vec<&str> {
    entries.iter().map(|entry| entry.text.as_str()).collect()
}
