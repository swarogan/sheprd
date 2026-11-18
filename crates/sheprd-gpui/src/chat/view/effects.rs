//! Message effects on screen: a bubble's Markdown in its pose, invisible
//! ink's specks, the link that plays an effect again, and a pane effect
//! painted over the chat. The pane effect is a child of the chat's own view,
//! which clips it, so it never reaches the terminals, tabs, or sidebar. It
//! takes no pointer events, so the chat stays usable while it plays.

use super::super::{
    chats::ChatPane,
    effects::{
        Bubble, Playing, Pose, Screen,
        scene::{self, BRIGHTS, GOLDS, Hue, Mark, Rect, Words},
    },
    rich::Block,
    visual::Visuals,
};
use super::{layout::Voice, selection::Scope};
use crate::i18n::fl;
use crate::{SheprdWindow, motion};
use gpui::{prelude::*, *};
use sheprd_chat_protocol::EntryId;

/// Golds a celebration's sparks take, whatever the theme.
const GOLD: [u32; GOLDS] = [0x00ff_d76a, 0x00ff_e9a8, 0x00ff_c23d, 0x00ff_f4d6];
/// The bright colors of zap's rows, by their order in the theme: yellow,
/// cyan, and magenta, which read on dark and light grounds.
const ROW_HUES: [usize; 3] = [2, 5, 4];
/// Specks of invisible ink per square point of the text they hide.
const INK_DENSITY: f32 = 1. / 18.;
/// The most specks one bubble paints.
const MAX_SPECKS: usize = 2_400;
/// How often the specks move while they shimmer, per second.
const SHIMMER_RATE: f32 = 14.;

/// Keeps frames coming while something it sits in moves.
fn next_frame() -> impl IntoElement {
    canvas(
        |_, _, _| {},
        |_, _, window, _| window.request_animation_frame(),
    )
    .absolute()
    .size_0()
}

impl SheprdWindow {
    /// Markdown an agent wrapped in a bubble effect, in the pose the effect
    /// has now, with the link that plays it again.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn render_chat_bubble(
        &self,
        pane_id: &str,
        entry: EntryId,
        order: usize,
        effect: Bubble,
        blocks: &[Block],
        visuals: &Visuals,
        scope: &Scope<'_>,
        voice: Voice,
        cx: &mut Context<Self>,
    ) -> Div {
        let since = self
            .chats
            .pane(pane_id)
            .and_then(|chat| chat.effects.bubble_since(entry, order))
            .filter(|_| !cx.reduce_motion());
        let t = since.map(|since| motion::now().saturating_duration_since(since).as_secs_f32());
        let pose = effect.pose(t);
        let voice = Voice {
            size: voice.size * pose.scale,
            ..voice
        };
        let content = self.render_chat_rich(pane_id, blocks, visuals, scope, voice, cx);
        let moving = pose != Pose::REST;
        // The specks move a step at a time while they shimmer, then stay.
        let frame = t
            .filter(|_| pose.shimmer)
            .map_or(0, |t| (t * SHIMMER_RATE) as u32 + 1);
        let body = match effect {
            Bubble::Ink => self.render_chat_ink(entry, order, content, frame),
            Bubble::Gentle | Bubble::Loud | Bubble::Slam => div()
                .relative()
                .left(px(pose.shift))
                .opacity(pose.opacity)
                .child(content),
        };
        div()
            .flex()
            .flex_col()
            .gap(px(2.))
            .min_w_0()
            .child(body.when(moving, |body| body.child(next_frame())))
            .when(effect != Bubble::Ink, |bubble| {
                let pane = pane_id.to_owned();
                bubble.child(self.render_chat_effect_replay(
                    ("chat-bubble-replay", order),
                    effect.label(),
                    move |chat| {
                        chat.effects.replay_bubble(entry, order, motion::now());
                    },
                    pane,
                    cx,
                ))
            })
    }

    /// Text under invisible ink: specks hide it until the pointer is over it.
    /// `frame` scatters the specks; it stays put once the effect is over,
    /// so a hidden message costs no frames.
    fn render_chat_ink(&self, entry: EntryId, order: usize, content: Div, frame: u32) -> Div {
        let group = SharedString::from(format!("chat-ink-{}-{order}", entry.0));
        let ink = rgb(self.theme.foreground);
        div()
            .debug_selector(|| "chat-ink".into())
            // Only as wide as its words, so the specks cover just them.
            .self_start()
            .max_w_full()
            .relative()
            .group(group.clone())
            .child(
                content
                    .opacity(0.)
                    .group_hover(group.clone(), |style| style.opacity(1.)),
            )
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .group_hover(group, |style| style.opacity(0.))
                    .child(
                        canvas(
                            |_, _, _| {},
                            move |bounds, _, window, _| paint_specks(bounds, frame, ink, window),
                        )
                        .size_full(),
                    ),
            )
    }

    /// The link that plays a message's effect again.
    pub(super) fn render_chat_screen_replay(
        &self,
        pane_id: &str,
        entry: EntryId,
        effect: Screen,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        self.render_chat_effect_replay(
            "chat-screen-replay",
            effect.label(),
            move |chat| {
                if let Some(body) = chat.prepared.body(entry) {
                    chat.effects.replay_screen(entry, &body, motion::now());
                }
            },
            pane_id.to_owned(),
            cx,
        )
    }

    fn render_chat_effect_replay(
        &self,
        id: impl Into<ElementId>,
        label: String,
        replay: impl Fn(&mut ChatPane) + 'static,
        pane: String,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let accent = self.theme.primary();
        div()
            .id(id)
            .debug_selector(|| "chat-effect-replay".into())
            .flex()
            .gap(px(4.))
            .text_size(self.chat_small_text())
            .text_color(rgb(self.theme.muted))
            .cursor_pointer()
            .hover(move |style| style.text_color(rgb(accent)))
            // The press would otherwise start a text selection.
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(move |this, _: &MouseUpEvent, _, cx| {
                    cx.stop_propagation();
                    if let Some(chat) = this.chats.pane_mut(&pane) {
                        replay(chat);
                        cx.notify();
                    }
                }),
            )
            .child("↻")
            .child(fl!("chat-effects-replay", effect = label))
    }

    /// The pane effect playing over the chat, if one is and motion is not
    /// reduced.
    pub(super) fn render_chat_screen(&self, chat: &ChatPane, cx: &App) -> Option<AnyElement> {
        if cx.reduce_motion() {
            return None;
        }
        let now = motion::now();
        let playing = chat.effects.playing(now)?.clone();
        let t = playing.elapsed(now)?;
        let target = chat.effects.target.clone();
        let palette = Palette {
            brights: std::array::from_fn(|i| rgb(self.theme.palette[9 + i]).into()),
            ink: rgb(self.theme.foreground).into(),
        };
        let font = self.config.ui.font();
        Some(
            canvas(
                |_, _, _| {},
                move |bounds, _, window, cx| {
                    let pane = Rect {
                        x: 0.,
                        y: 0.,
                        w: bounds.size.width.into(),
                        h: bounds.size.height.into(),
                    };
                    let target = target.get().map(|target| Rect {
                        x: f32::from(target.origin.x - bounds.origin.x),
                        y: f32::from(target.origin.y - bounds.origin.y),
                        w: target.size.width.into(),
                        h: target.size.height.into(),
                    });
                    let mut marks = Vec::new();
                    let letters = playing
                        .text
                        .chars()
                        .filter(|ch| !ch.is_whitespace())
                        .count();
                    scene::scene(playing.effect, t, pane, target, letters, &mut marks);
                    for mark in marks {
                        paint(&mark, bounds, &playing, &palette, &font, window, cx);
                    }
                    window.request_animation_frame();
                },
            )
            .absolute()
            .inset_0()
            .into_any_element(),
        )
    }
}

/// The colors a pane effect paints with.
struct Palette {
    brights: [Hsla; BRIGHTS],
    ink: Hsla,
}

impl Palette {
    fn hue(&self, hue: Hue) -> Hsla {
        match hue {
            Hue::Bright(index) => self.brights[index % BRIGHTS],
            Hue::Gold(index) => rgb(GOLD[index % GOLDS]).into(),
            Hue::White => white(),
        }
    }
}

fn faded(color: Hsla, alpha: f32) -> Hsla {
    color.opacity(alpha.clamp(0., 1.))
}

fn at(origin: Point<Pixels>, x: f32, y: f32) -> Point<Pixels> {
    point(origin.x + px(x), origin.y + px(y))
}

fn paint(
    mark: &Mark,
    bounds: Bounds<Pixels>,
    playing: &Playing,
    palette: &Palette,
    font: &Font,
    window: &mut Window,
    cx: &mut App,
) {
    let origin = bounds.origin;
    match *mark {
        Mark::Box {
            rect,
            round,
            hue,
            alpha,
        } => window.paint_quad(
            fill(
                Bounds::new(at(origin, rect.x, rect.y), size(px(rect.w), px(rect.h))),
                faded(palette.hue(hue), alpha),
            )
            .corner_radii(px(round)),
        ),
        Mark::Beam {
            from,
            to,
            width,
            hue,
            alpha,
        } => {
            let mut path = PathBuilder::stroke(px(width));
            path.move_to(at(origin, from.0, from.1));
            path.line_to(at(origin, to.0, to.1));
            if let Ok(path) = path.build() {
                window.paint_path(path, faded(palette.hue(hue), alpha));
            }
        }
        Mark::Words {
            x,
            y,
            size,
            alpha,
            words,
        } => {
            let (text, color) = match words {
                Words::Message => (playing.text.as_str(), palette.ink),
                Words::Heart => ("❤️", white()),
                Words::SmallHeart => ("💗", white()),
                Words::Letter { index, row } => (
                    letter(&playing.text, index),
                    palette.hue(Hue::Bright(ROW_HUES[row % ROW_HUES.len()])),
                ),
            };
            if text.is_empty() {
                return;
            }
            // Lasers draw the words in white over their dark veil.
            let color = match (words, playing.effect) {
                (Words::Message, Screen::Lasers) => white(),
                _ => color,
            };
            let run = TextRun {
                len: text.len(),
                font: font.clone(),
                color: faded(color, alpha),
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let line = window.text_system().shape_line(
                SharedString::from(text.to_owned()),
                px(size),
                &[run],
                None,
            );
            let left = x - f32::from(line.width) / 2.;
            let _ = line.paint(
                at(origin, left, y),
                px(size * 1.2),
                TextAlign::Left,
                None,
                window,
                cx,
            );
        }
        Mark::Veil { alpha, hole } => {
            let dark = faded(black(), alpha);
            let Some(hole) = hole else {
                window.paint_quad(fill(bounds, dark));
                return;
            };
            let (w, h) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
            let (left, top) = (hole.x.clamp(0., w), hole.y.clamp(0., h));
            let (right, bottom) = (
                (hole.x + hole.w).clamp(0., w),
                (hole.y + hole.h).clamp(0., h),
            );
            for (x, y, width, height) in [
                (0., 0., w, top),
                (0., bottom, w, h - bottom),
                (0., top, left, bottom - top),
                (right, top, w - right, bottom - top),
            ] {
                if width > 0. && height > 0. {
                    window.paint_quad(fill(
                        Bounds::new(at(origin, x, y), size(px(width), px(height))),
                        dark,
                    ));
                }
            }
        }
    }
}

/// The `index`th letter of `text` that is not a space, or nothing.
fn letter(text: &str, index: usize) -> &str {
    text.char_indices()
        .filter(|(_, ch)| !ch.is_whitespace())
        .nth(index)
        .map_or("", |(at, ch)| &text[at..at + ch.len_utf8()])
}

/// Invisible ink's specks over `bounds`, scattered for `frame`.
fn paint_specks(bounds: Bounds<Pixels>, frame: u32, ink: Rgba, window: &mut Window) {
    let (w, h) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
    let count = ((w * h * INK_DENSITY) as usize).min(MAX_SPECKS);
    let ink: Hsla = ink.into();
    for index in 0..count as u32 {
        let (x, y, alpha) = scene::speck(index, frame);
        window.paint_quad(fill(
            Bounds::new(at(bounds.origin, x * w, y * h), size(px(1.5), px(1.5))),
            faded(ink, alpha * 0.85),
        ));
    }
}
