//! Message effects an agent asks for, as a phone's messages have: a bubble
//! effect plays on the Markdown between `:::slam` and `:::`, and a pane
//! effect, asked for by a `::confetti` line, fills the chat's own pane for a
//! few seconds and no more. Only an agent's reply plays one.
//!
//! An effect plays once, as its message arrives. A transcript the chat
//! loads, opens again, or receives again on reconnecting shows each message
//! as it ends, with a link that plays its effect again. With the system's
//! reduce motion on, every effect is drawn at rest, and invisible ink still
//! hides its text until the pointer is over it.

use super::prepared::{Body, Part};
use crate::i18n::fl;
use gpui::{Bounds, Pixels};
use sheprd_chat_protocol::EntryId;
use std::{
    cell::Cell,
    collections::HashMap,
    rc::Rc,
    time::{Duration, Instant},
};

mod pose;
pub(crate) mod scene;

pub(crate) use pose::Pose;

/// Declares each effect once: its variant, the name a directive uses, the
/// label its replay link shows, and how long it plays.
macro_rules! effects {
    ($(#[$meta:meta])* $kind:ident {
        $($variant:ident = $name:literal, $label:literal, $millis:literal;)+
    }) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub(crate) enum $kind {
            $($variant,)+
        }

        impl $kind {
            pub(crate) const ALL: &'static [Self] = &[$(Self::$variant,)+];

            pub(crate) fn name(self) -> &'static str {
                match self {
                    $(Self::$variant => $name,)+
                }
            }

            pub(crate) fn label(self) -> String {
                match self {
                    $(Self::$variant => fl!($label),)+
                }
            }

            /// How long the effect plays, before any slow motion.
            pub(crate) fn length(self) -> Duration {
                match self {
                    $(Self::$variant => Duration::from_millis($millis),)+
                }
            }

            fn named(name: &str) -> Option<Self> {
                Self::ALL.iter().copied().find(|effect| effect.name() == name)
            }
        }
    };
}

effects! {
    /// An effect on the Markdown it wraps.
    Bubble {
        // keep-sorted start
        Gentle = "gentle", "chat-effects-gentle", 2200;
        Ink = "ink", "chat-effects-ink", 4000;
        Loud = "loud", "chat-effects-loud", 1600;
        Slam = "slam", "chat-effects-slam", 900;
        // keep-sorted end
    }
}

effects! {
    /// An effect over the chat's whole pane, and nothing beyond it.
    Screen {
        // keep-sorted start
        Balloons = "balloons", "chat-effects-balloons", 4200;
        Celebration = "celebration", "chat-effects-celebration", 3400;
        Confetti = "confetti", "chat-effects-confetti", 3400;
        Echo = "echo", "chat-effects-echo", 3800;
        Fireworks = "fireworks", "chat-effects-fireworks", 4000;
        Lasers = "lasers", "chat-effects-lasers", 3600;
        Love = "love", "chat-effects-love", 3000;
        Spotlight = "spotlight", "chat-effects-spotlight", 3300;
        Zap = "zap", "chat-effects-zap", 4600;
        // keep-sorted end
    }
}

/// The bubble effect a line such as `:::slam` opens.
pub(super) fn bubble_opening(line: &str) -> Option<Bubble> {
    Bubble::named(line.trim().strip_prefix(":::")?)
}

/// Whether a line is the `:::` that ends a bubble effect.
pub(super) fn bubble_closing(line: &str) -> bool {
    line.trim() == ":::"
}

/// The pane effect a line such as `::confetti` asks for.
pub(super) fn screen_line(line: &str) -> Option<Screen> {
    let name = line.trim().strip_prefix("::")?;
    // `:::slam` is a bubble's opening, not a pane effect named `:slam`.
    if name.starts_with(':') {
        return None;
    }
    Screen::named(name)
}

/// The effects a prepared message holds: its bubbles, in order, and its
/// pane effect.
fn found(body: &Body) -> (usize, Option<Screen>) {
    let Body::Message(parts) = body else {
        return (0, None);
    };
    parts
        .iter()
        .fold((0, None), |(bubbles, screen), part| match part {
            Part::Bubble(..) => (bubbles + 1, screen),
            Part::Screen(effect) => (bubbles, screen.or(Some(*effect))),
            _ => (bubbles, screen),
        })
}

/// What the message's text is, in brief, for effects that draw it: its
/// first paragraph's words.
fn gist(body: &Body) -> String {
    let Body::Message(parts) = body else {
        return String::new();
    };
    let text: String = parts
        .iter()
        .filter_map(|part| match part {
            Part::Rich(blocks) | Part::Bubble(_, blocks) => Some(blocks),
            _ => None,
        })
        .flat_map(|blocks| blocks.iter())
        .find_map(|block| match block {
            super::rich::Block::Paragraph { content, .. } => Some(
                content
                    .iter()
                    .filter_map(|part| match part {
                        super::rich::Inline::Text(text, _) => Some(text.as_str()),
                        _ => None,
                    })
                    .collect(),
            ),
            _ => None,
        })
        .unwrap_or_default();
    crate::notifications::safe_text(text.trim(), GIST)
}

/// Characters of a message an effect draws.
const GIST: usize = 48;

/// A pane effect playing.
#[derive(Clone, Debug)]
pub(crate) struct Playing {
    pub(crate) entry: EntryId,
    pub(crate) effect: Screen,
    pub(crate) since: Instant,
    /// The message's words, for the effects that draw them.
    pub(crate) text: String,
}

impl Playing {
    /// How far into the effect `now` is, in seconds, or `None` once it ended.
    pub(crate) fn elapsed(&self, now: Instant) -> Option<f32> {
        // `now` is the motion clock, which slow motion already slows.
        let elapsed = now.saturating_duration_since(self.since);
        (elapsed < self.effect.length()).then_some(elapsed.as_secs_f32())
    }
}

/// The effects of one chat: which messages' effects were met, and which
/// play now.
#[derive(Debug, Default)]
pub(crate) struct Effects {
    /// The revision of each entry last looked at, so a live update looks
    /// only at the entries it changed, wherever they are.
    revisions: HashMap<EntryId, u64>,
    /// Bubbles met in each message, so only a new one plays.
    bubbles: HashMap<EntryId, usize>,
    /// Messages whose pane effect was met.
    screens: HashMap<EntryId, Screen>,
    /// When each bubble last began to play, by message and its order there.
    started: HashMap<(EntryId, usize), Instant>,
    playing: Option<Playing>,
    /// Where the message a spotlight looks for was last drawn, in the
    /// window, while one plays.
    pub(crate) target: Rc<Cell<Option<Bounds<Pixels>>>>,
}

impl Effects {
    /// Meets the effects of the transcript's messages. `history` is a whole
    /// transcript loaded at once, whose effects are only noted; otherwise
    /// the effects new in the entries it changed play. With reduce motion on, the view
    /// draws each at rest.
    pub(crate) fn sync(
        &mut self,
        bodies: impl Fn(EntryId) -> Option<Rc<Body>>,
        keys: &[(EntryId, u64)],
        history: bool,
        now: Instant,
    ) {
        if history {
            self.revisions.clear();
            self.bubbles.clear();
            self.screens.clear();
            self.started.clear();
            self.playing = None;
        }
        for (entry, revision) in keys {
            if self.revisions.insert(*entry, *revision) == Some(*revision) {
                continue;
            }
            let Some(body) = bodies(*entry) else {
                continue;
            };
            // The words an effect draws stream in after its directive.
            if let Some(playing) = self.playing.as_mut()
                && playing.entry == *entry
            {
                playing.text = gist(&body);
            }
            let (bubbles, screen) = found(&body);
            let met = self.bubbles.get(entry).copied().unwrap_or(0);
            if bubbles > met {
                self.bubbles.insert(*entry, bubbles);
                if !history {
                    for order in met..bubbles {
                        self.started.insert((*entry, order), now);
                    }
                }
            }
            if let Some(effect) = screen
                && !self.screens.contains_key(entry)
            {
                self.screens.insert(*entry, effect);
                // One at a time: another arriving while one plays is dropped.
                if !history && !self.is_playing(now) {
                    self.target.set(None);
                    self.playing = Some(Playing {
                        entry: *entry,
                        effect,
                        since: now,
                        text: gist(&body),
                    });
                }
            }
        }
        self.started
            .retain(|_, since| now.saturating_duration_since(*since) < LONGEST);
    }

    /// Plays a message's bubble again, as its replay link asks.
    pub(crate) fn replay_bubble(&mut self, entry: EntryId, order: usize, now: Instant) {
        self.started.insert((entry, order), now);
    }

    /// Plays a message's pane effect again, in place of one playing.
    pub(crate) fn replay_screen(&mut self, entry: EntryId, body: &Body, now: Instant) {
        if let (_, Some(effect)) = found(body) {
            self.target.set(None);
            self.playing = Some(Playing {
                entry,
                effect,
                since: now,
                text: gist(body),
            });
        }
    }

    /// When the message's `order`th bubble began to play, if it may still.
    pub(crate) fn bubble_since(&self, entry: EntryId, order: usize) -> Option<Instant> {
        self.started.get(&(entry, order)).copied()
    }

    /// The pane effect playing at `now`.
    pub(crate) fn playing(&self, now: Instant) -> Option<&Playing> {
        self.playing
            .as_ref()
            .filter(|playing| playing.elapsed(now).is_some())
    }

    fn is_playing(&self, now: Instant) -> bool {
        self.playing(now).is_some()
    }
}

/// Longer than any bubble plays, even slowed, after which its start is
/// forgotten.
const LONGEST: Duration = Duration::from_secs(60);

#[cfg(test)]
mod tests;
