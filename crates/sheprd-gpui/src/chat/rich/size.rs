//! Text an agent sets larger or smaller within a message, as `:big[text]`
//! asks. The agent names one of a few steps, never a size of its own, so one
//! word cannot fill the pane. A name that is not a step stays text.

/// How much larger or smaller than the prose a span reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Size {
    Small,
    Big,
    Huge,
    Giant,
}

impl Size {
    const ALL: [Self; 4] = [Self::Small, Self::Big, Self::Huge, Self::Giant];

    fn name(self) -> &'static str {
        match self {
            Self::Small => "small",
            Self::Big => "big",
            Self::Huge => "huge",
            Self::Giant => "giant",
        }
    }

    /// The span's text size, as a multiple of the prose's.
    pub(crate) fn scale(self) -> f32 {
        match self {
            Self::Small => 0.85,
            Self::Big => 1.35,
            Self::Huge => 2.,
            Self::Giant => 3.,
        }
    }

    /// Whether the span reads heavier than the prose, as larger text does.
    pub(crate) fn heavy(self) -> bool {
        self != Self::Small
    }

    /// The span's line height, in ems: larger text keeps its lines tighter.
    pub(crate) fn line_height(self) -> f32 {
        match self {
            Self::Small => 1.6,
            Self::Big => 1.3,
            Self::Huge | Self::Giant => 1.15,
        }
    }

    /// The step `rest` opens, as `big[` does, with the bytes it takes.
    fn opening(rest: &str) -> Option<(Self, usize)> {
        Self::ALL.into_iter().find_map(|size| {
            let name = size.name();
            (rest.starts_with(name) && rest[name.len()..].starts_with('['))
                .then_some((size, name.len() + 1))
        })
    }

    /// The step a text ending in `:big` names, with the bytes the name
    /// takes, for an opening split from its `[` across Markdown events.
    pub(super) fn ending(text: &str) -> Option<(Self, usize)> {
        Self::ALL.into_iter().find_map(|size| {
            let start = text.strip_suffix(size.name())?.strip_suffix(':')?;
            starts_directive(start).then_some((size, size.name().len() + 1))
        })
    }
}

/// Whether a directive may begin after `before`: at the start, or after a
/// character that is not part of a word, so `http://x:big[` stays text.
fn starts_directive(before: &str) -> bool {
    before
        .chars()
        .next_back()
        .is_none_or(|ch| !ch.is_alphanumeric() && ch != '/')
}

/// Spans open around the text being read, innermost last, each with how
/// many of its own brackets are open inside it.
#[derive(Debug, Default)]
pub(super) struct Sizes {
    open: Vec<(Size, usize)>,
}

/// Spans open at once; deeper openings stay text.
const MAX_OPEN: usize = 8;

impl Sizes {
    /// The size text reads at now.
    pub(super) fn current(&self) -> Option<Size> {
        self.open.last().map(|(size, _)| *size)
    }

    /// Opens a span. `false` when too many are open already.
    pub(super) fn open(&mut self, size: Size) -> bool {
        if self.open.len() >= MAX_OPEN {
            return false;
        }
        self.open.push((size, 1));
        true
    }

    /// Ends every span, as a paragraph or cell ends: an unclosed span
    /// never reaches into the next block.
    pub(super) fn close_all(&mut self) {
        self.open.clear();
    }

    /// Splits `text` at the spans it opens and closes, handing each piece
    /// with the size it reads at to `push`. The directive's own marks are
    /// left out; every other byte is handed on once.
    pub(super) fn split<'t>(&mut self, text: &'t str, mut push: impl FnMut(&'t str, Option<Size>)) {
        let mut piece = 0;
        let mut chars = text.char_indices();
        while let Some((at, ch)) = chars.next() {
            let skip = match ch {
                ':' if starts_directive(&text[..at]) => match Size::opening(&text[at + 1..]) {
                    Some((size, len)) if self.open.len() < MAX_OPEN => {
                        push(&text[piece..at], self.current());
                        self.open.push((size, 1));
                        Some(1 + len)
                    }
                    _ => None,
                },
                '[' => {
                    if let Some((_, depth)) = self.open.last_mut() {
                        *depth += 1;
                    }
                    None
                }
                ']' => match self.open.last_mut() {
                    Some((_, depth)) if *depth == 1 => {
                        push(&text[piece..at], self.current());
                        self.open.pop();
                        Some(1)
                    }
                    Some((_, depth)) => {
                        *depth -= 1;
                        None
                    }
                    None => None,
                },
                _ => None,
            };
            if let Some(len) = skip {
                piece = at + len;
                // The marks are ASCII, so the rest of them are single bytes.
                for _ in 1..len {
                    chars.next();
                }
            }
        }
        push(&text[piece..], self.current());
    }
}
