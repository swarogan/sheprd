use super::*;
use crate::chat::{
    effects::{Bubble, Effects, Screen},
    prepared::{Body, Part, Prepared},
};
use std::time::Duration;

fn shape(text: &str) -> Vec<String> {
    visual::message(text).1
}

#[test]
fn a_bubble_effect_wraps_the_markdown_between_its_fences() {
    assert_eq!(
        shape("Before\n:::slam\nFound **it**.\n:::\nAfter"),
        ["text:Before", "bubble:slam", "text:After"]
    );
}

#[test]
fn a_bubble_still_streaming_reaches_the_end_of_the_message() {
    assert_eq!(shape("Hi\n:::ink\nThe key is"), ["text:Hi", "bubble:ink"]);
    // Only its opening so far: nothing to play yet.
    assert_eq!(shape("Hi\n:::loud\n"), ["text:Hi"]);
}

#[test]
fn a_pane_effect_line_is_taken_out_and_its_link_ends_the_message() {
    assert_eq!(
        shape("::confetti\nAll tests pass.\n::fireworks"),
        ["text:All tests pass.", "screen:confetti"]
    );
}

#[test]
fn unknown_or_indented_or_fenced_directives_stay_text() {
    assert_eq!(shape("::sparkles\nHi"), ["text:::sparkles Hi"]);
    assert_eq!(shape(":::wobble\nHi\n:::"), ["text::::wobble Hi :::"]);
    let fenced = shape("```\n::confetti\n```");
    assert!(
        !fenced.iter().any(|part| part.starts_with("screen")),
        "{fenced:?}"
    );
    let indented = shape("    ::confetti");
    assert!(
        !indented.iter().any(|part| part.starts_with("screen")),
        "{indented:?}"
    );
}

#[test]
fn a_user_message_never_plays_an_effect() {
    let mut transcript = Transcript::new("Claude");
    transcript.entries = vec![Entry {
        id: EntryId(1),
        kind: EntryKind::User {
            text: "::confetti\n:::slam\nme\n:::".into(),
            files: 0,
        },
    }];
    let mut prepared = Prepared::default();
    prepared.update(&transcript, &[(EntryId(1), 1)]);
    let Body::Message(parts) = &*prepared.body(EntryId(1)).unwrap() else {
        panic!("message");
    };
    assert!(
        parts
            .iter()
            .all(|part| !matches!(part, Part::Bubble(..) | Part::Screen(_)))
    );
}

#[test]
fn every_effect_has_a_directive_name_and_label() {
    for effect in Bubble::ALL {
        assert!(
            super::super::effects::bubble_opening(&format!(":::{}", effect.name()))
                == Some(*effect)
        );
        assert!(!effect.label().is_empty());
    }
    for effect in Screen::ALL {
        assert!(
            super::super::effects::screen_line(&format!("::{}", effect.name())) == Some(*effect)
        );
        assert!(!effect.label().is_empty());
    }
}

/// A transcript of agent messages, prepared, with its keys.
fn prepared(texts: &[&str]) -> (Prepared, Vec<(EntryId, u64)>) {
    let mut transcript = Transcript::new("Claude");
    transcript.entries = texts
        .iter()
        .enumerate()
        .map(|(index, text)| agent(index as u64 + 1, text))
        .collect();
    // A changed text is a new revision, as the reducer makes it.
    let keys: Vec<_> = texts
        .iter()
        .enumerate()
        .map(|(index, text)| (EntryId(index as u64 + 1), text.len() as u64))
        .collect();
    let mut prepared = Prepared::default();
    prepared.update(&transcript, &keys);
    (prepared, keys)
}

fn sync(effects: &mut Effects, texts: &[&str], history: bool, now: Instant) {
    let (prepared, keys) = prepared(texts);
    effects.sync(|entry| prepared.body(entry), &keys, history, now);
}

#[test]
fn a_loaded_transcript_notes_its_effects_without_playing_them() {
    let now = Instant::now();
    let mut effects = Effects::default();
    sync(&mut effects, &[":::slam\nhi\n:::\n::confetti"], true, now);
    assert!(effects.playing(now).is_none());
    assert!(effects.bubble_since(EntryId(1), 0).is_none());
    // Seen again, as a reconnect sends it, it still does not play.
    sync(&mut effects, &[":::slam\nhi\n:::\n::confetti"], false, now);
    assert!(effects.playing(now).is_none());
    assert!(effects.bubble_since(EntryId(1), 0).is_none());
}

#[test]
fn a_new_message_plays_its_effects_once() {
    let now = Instant::now();
    let mut effects = Effects::default();
    sync(&mut effects, &["earlier"], true, now);
    let live = ["earlier", ":::slam\nFound it\n:::\n::confetti"];
    sync(&mut effects, &live, false, now);
    let playing = effects.playing(now).expect("confetti plays");
    assert_eq!(
        (playing.entry, playing.effect),
        (EntryId(2), Screen::Confetti)
    );
    assert_eq!(playing.text, "Found it");
    assert_eq!(effects.bubble_since(EntryId(2), 0), Some(now));
    // The next chunk of the same message does not start them again.
    let later = now + Duration::from_millis(100);
    sync(&mut effects, &live, false, later);
    assert_eq!(effects.playing(later).unwrap().since, now);
    assert_eq!(effects.bubble_since(EntryId(2), 0), Some(now));
    // Once over, it stays over.
    let after = now + Screen::Confetti.length();
    assert!(effects.playing(after).is_none());
}

#[test]
fn a_bubble_that_streams_in_later_plays_on_its_own() {
    let now = Instant::now();
    let later = now + Duration::from_millis(500);
    let mut effects = Effects::default();
    sync(&mut effects, &[], true, now);
    sync(&mut effects, &[":::slam\nOne\n:::\nthen"], false, now);
    sync(
        &mut effects,
        &[":::slam\nOne\n:::\nthen\n:::loud\nTwo"],
        false,
        later,
    );
    assert_eq!(effects.bubble_since(EntryId(1), 0), Some(now));
    assert_eq!(effects.bubble_since(EntryId(1), 1), Some(later));
}

#[test]
fn a_pane_effect_arriving_while_one_plays_is_dropped() {
    let now = Instant::now();
    let mut effects = Effects::default();
    sync(&mut effects, &[], true, now);
    sync(&mut effects, &["::confetti\nOne"], false, now);
    let soon = now + Duration::from_millis(200);
    sync(
        &mut effects,
        &["::confetti\nOne", "::lasers\nTwo"],
        false,
        soon,
    );
    assert_eq!(effects.playing(soon).unwrap().effect, Screen::Confetti);
    // It is not queued: the confetti ends and nothing follows.
    assert!(effects.playing(now + Screen::Confetti.length()).is_none());
}

#[test]
fn a_replay_plays_an_effect_again() {
    let now = Instant::now();
    let (prepared, keys) = prepared(&[":::loud\nHey\n:::\n::love"]);
    let mut effects = Effects::default();
    effects.sync(|entry| prepared.body(entry), &keys, true, now);
    let later = now + Duration::from_secs(30);
    effects.replay_bubble(EntryId(1), 0, later);
    assert_eq!(effects.bubble_since(EntryId(1), 0), Some(later));
    effects.replay_screen(EntryId(1), &prepared.body(EntryId(1)).unwrap(), later);
    assert_eq!(effects.playing(later).unwrap().effect, Screen::Love);
}

#[test]
fn a_fenced_closing_line_stays_in_the_bubble() {
    assert_eq!(
        shape(":::slam\n```markdown\n:::\n```\nafter\n:::\nOutside"),
        ["bubble:slam", "text:Outside"]
    );
}

#[test]
fn an_effect_followed_by_many_entries_in_one_update_still_plays() {
    let now = Instant::now();
    let mut effects = Effects::default();
    sync(&mut effects, &["earlier"], true, now);
    let mut live = vec!["earlier", ":::loud\nDone\n:::\n::confetti"];
    live.extend(["step"; 8]);
    sync(&mut effects, &live, false, now);
    assert_eq!(effects.playing(now).unwrap().entry, EntryId(2));
    assert_eq!(effects.bubble_since(EntryId(2), 0), Some(now));
}

#[test]
fn the_words_an_effect_draws_follow_its_message_as_it_streams() {
    let now = Instant::now();
    let mut effects = Effects::default();
    sync(&mut effects, &[], true, now);
    let (first, keys) = prepared(&["::echo\n"]);
    effects.sync(|entry| first.body(entry), &keys, false, now);
    assert_eq!(effects.playing(now).unwrap().text, "");
    let (grown, grown_keys) = prepared(&["::echo\nMerged!"]);
    effects.sync(|entry| grown.body(entry), &grown_keys, false, now);
    let playing = effects.playing(now).unwrap();
    assert_eq!((playing.text.as_str(), playing.since), ("Merged!", now));
}
