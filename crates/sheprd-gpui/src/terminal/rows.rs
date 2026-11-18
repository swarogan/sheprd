//! Which rows of a box hold terminal cells.
//!
//! A box rarely stands a whole number of rows tall. The remainder is under one
//! row, and where it falls decides what the eye sees: below the last row, it
//! looks like a gap under the prompt; above the first, it is a thin margin
//! that reads as the window's own edge. [`bottom_rows`] puts it above.

use gpui::{
    AnyElement, App, AvailableSpace, Bounds, ContentMask, Element, ElementId, GlobalElementId,
    InspectorElementId, IntoElement, LayoutId, Pixels, Style, Window, point, px, relative, size,
};

/// Fractions of a row a box may fall short of a whole row and still count as
/// holding it. A box sized to N rows comes back through float and device-pixel
/// rounding a hair under N rows tall, and must not lose one.
const SLACK: f32 = 1e-3;

/// Device pixels within which a row edge counts as half a pixel off.
const TIE: f32 = 1e-3;

/// How many whole rows of `cell_height` fit in `height`.
pub(crate) fn whole_rows(height: f32, cell_height: f32) -> f32 {
    (height / cell_height.max(1.) + SLACK).floor()
}

/// The box the bottom-aligned whole rows of `bounds` are laid out in, at
/// `scale` device pixels per logical pixel. A box under one row tall is left
/// as it is.
///
/// GPUI puts the box's origin on a whole device pixel and the painter rounds
/// each row edge to one, half toward zero, so the top is chosen for the last
/// row's edge to round onto `bounds`' bottom. The height is rounded up rather
/// than to that edge, so the content never reads back a row short; it can
/// reach up to one device pixel past the bottom, which [`bottom_rows`] clips.
/// A last row ending half a device pixel from the bottom rounds either way
/// through float error, so it is put past the bottom, where the clip hides
/// the extra pixel, rather than short of it, where a gap would show.
pub(crate) fn bottom_aligned(
    bounds: Bounds<Pixels>,
    cell_height: f32,
    scale: f32,
) -> Bounds<Pixels> {
    let height = f32::from(bounds.size.height);
    let rows = whole_rows(height, cell_height);
    if rows < 1. {
        return bounds;
    }
    let grid = rows * cell_height.max(1.) * scale;
    let lift = (grid - 0.5 - TIE).ceil().min(height * scale);
    Bounds::new(
        point(bounds.left(), bounds.bottom() - px(lift / scale)),
        size(bounds.size.width, px(grid.ceil() / scale)),
    )
}

/// Lays `content` out in the whole rows at the bottom of the box it sits in,
/// so the sub-row remainder is above its first row. The cells' origin, hit
/// testing, painting, and the rows the daemon is asked for all read the
/// content's bounds, so they agree. Nothing the content draws reaches below
/// the box.
pub(crate) fn bottom_rows(cell_height: f32, content: impl IntoElement) -> BottomRows {
    BottomRows {
        cell_height,
        content: content.into_any_element(),
    }
}

pub(crate) struct BottomRows {
    cell_height: f32,
    content: AnyElement,
}

impl IntoElement for BottomRows {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for BottomRows {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        // The box the content is fitted into. The content is laid out on its
        // own once this box's size is known, so it is no child of this node.
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = relative(1.).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let grid = bottom_aligned(bounds, self.cell_height, window.scale_factor());
        window.with_content_mask(Some(above(bounds, window)), |window| {
            self.content.prepaint_as_root(
                grid.origin,
                grid.size.map(AvailableSpace::Definite),
                window,
                cx,
            );
        });
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        window.with_content_mask(Some(above(bounds, window)), |window| {
            self.content.paint(window, cx);
        });
    }
}

/// The window's current content mask, cut off at the bottom of `bounds`.
fn above(bounds: Bounds<Pixels>, window: &Window) -> ContentMask<Pixels> {
    let mask = window.content_mask().bounds;
    ContentMask {
        bounds: Bounds::from_corners(
            mask.origin,
            point(mask.right(), mask.bottom().min(bounds.bottom())),
        ),
    }
}
