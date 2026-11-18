use super::*;
use crate::{
    config::{ClipboardToastPosition, Padding, Sides},
    terminal::viewport,
    window::Flash,
};
use gpui::{Bounds, Pixels, px, size};
use std::time::{Duration, Instant};

fn draw(cx: &mut gpui::VisualTestContext) {
    cx.update(|window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    });
}

/// Device pixels per logical pixel in the test window.
pub(super) fn scale(cx: &mut gpui::VisualTestContext) -> f32 {
    cx.update(|window, _| window.scale_factor())
}

/// The cells end where the bottom padding begins, and so does the last row
/// the painter draws from their top, once rounded to a device pixel at
/// `scale`. The cells hold whole rows, reaching up to a device pixel past
/// the edge where a row is a fraction of a pixel tall; what is left of the
/// terminal above them is under one row.
pub(super) fn assert_bottom_aligned(
    cells: Bounds<Pixels>,
    terminal: Bounds<Pixels>,
    padding: Padding,
    row: f32,
    scale: f32,
) {
    let device = |logical: Pixels| {
        let device = f32::from(logical) * scale;
        (device.abs() - 0.5).ceil().copysign(device)
    };
    let bottom = terminal.bottom() - px(padding.y.end());
    let rows = (f32::from(cells.size.height) / row + 1e-3).floor();
    // A last row ending exactly half a device pixel past the edge may round
    // one further, which the clip at the edge hides; never short of it.
    let painted = device(cells.top() + px(rows * row));
    let past = f32::from(cells.top() + px(rows * row) - bottom) * scale;
    let tie = (past - 0.5).abs() < 1e-3;
    assert!(
        painted == device(bottom) || (tie && painted == device(bottom) + 1.),
        "{rows} rows of {row} from {:?} end at {painted}",
        cells.top()
    );
    let overhang = (cells.bottom() - bottom) * scale;
    assert!(
        overhang > px(-1e-3) && overhang < px(1. + 1e-3),
        "{overhang:?} past the bottom"
    );
    let above = cells.top() - (terminal.top() + px(padding.y.start()));
    assert!(
        above >= px(-1e-3) && above < px(row),
        "{above:?} above the rows"
    );
}

#[gpui::test]
fn terminal_padding_comes_out_of_all_four_sides_of_the_terminal(cx: &mut gpui::TestAppContext) {
    let (view, cx) = cx.add_window_view(fixture_window);
    cx.simulate_resize(size(px(900.), px(600.)));
    let set = |cx: &mut gpui::VisualTestContext, padding: Padding, sidebar: bool| {
        cx.update(|_, cx| {
            view.update(cx, |view, cx| {
                view.config.terminal_padding = padding;
                view.sidebar_visible = sidebar;
                cx.notify();
            });
        });
    };

    // The shipped default puts the cells against the left, right, and bottom
    // edges of the group, with under a row of terminal above them.
    draw(cx);
    let sidebar = cx.debug_bounds("sidebar").unwrap();
    let outer = cx.debug_bounds("terminal").unwrap();
    let flush = view.read_with(cx, |view, _| view.bounds);
    let row = view.read_with(cx, |view, _| view.config.terminal.line_height());
    assert_eq!((flush.left(), flush.right()), (outer.left(), outer.right()));
    assert_bottom_aligned(flush, outer, Padding::default(), row, scale(cx));
    assert!(flush.size.height < outer.size.height, "a remainder to move");
    assert_eq!(flush.origin.x, sidebar.right());

    let padding = Padding {
        x: Sides::new(16., 8.).unwrap(),
        y: Sides::new(4., 12.).unwrap(),
    };
    set(cx, padding, true);
    draw(cx);
    // Padding moves nothing around the terminal.
    assert_eq!(cx.debug_bounds("sidebar").unwrap(), sidebar);
    assert_eq!(cx.debug_bounds("terminal").unwrap(), outer);
    // The same bounds feed painting, hit testing, and the resize the daemon
    // sees, so the padding must come out of the terminal's own size.
    let padded = view.read_with(cx, |view, _| view.bounds);
    assert_eq!(padded.origin.x, flush.origin.x + px(16.));
    assert_eq!(padded.size.width, flush.size.width - px(16. + 8.));
    assert_bottom_aligned(padded, outer, padding, row, scale(cx));
    view.read_with(cx, |view, _| {
        assert_eq!(
            view.options.surface_size,
            viewport(
                f32::from(padded.size.width),
                f32::from(padded.size.height),
                view.cell_width,
                view.config.terminal.line_height(),
            )
        );
    });

    // Unlike the gap it replaces, padding does not depend on the sidebar.
    cx.update(|_, cx| {
        view.update(cx, |view, _| {
            view.settings.shared = Some(
                crate::herdr_settings::Settings::parse_text(
                    "[ui]\nsidebar_collapsed_mode = 'hidden'\n",
                )
                .unwrap(),
            );
        });
    });
    set(cx, padding, false);
    draw(cx);
    assert!(cx.debug_bounds("sidebar").is_none());
    let outer = cx.debug_bounds("terminal").unwrap();
    let hidden = view.read_with(cx, |view, _| view.bounds);
    assert_eq!(outer.origin.x, px(0.));
    assert_eq!(hidden.origin.x, px(16.));
    assert_eq!(hidden.size.width, outer.size.width - px(16. + 8.));
    assert_bottom_aligned(hidden, outer, padding, row, scale(cx));
}

/// The flash reports on the cells, so a corner position is a corner of the
/// cells, not of the padding around them.
#[gpui::test]
fn clipboard_flash_sits_over_the_cells_inside_the_terminal_padding(cx: &mut gpui::TestAppContext) {
    let (view, cx) = cx.add_window_view(fixture_window);
    cx.simulate_resize(size(px(900.), px(600.)));
    let show = |cx: &mut gpui::VisualTestContext, position| {
        cx.update(|_, cx| {
            view.update(cx, |view, cx| {
                view.config.terminal_padding = Padding {
                    x: Sides::new(16., 8.).unwrap(),
                    y: Sides::new(4., 12.).unwrap(),
                };
                view.config.clipboard_toast.position = position;
                view.flash = Some((
                    Flash::success("copied to clipboard"),
                    Instant::now() + Duration::from_secs(60),
                ));
                cx.notify();
            });
        });
        draw(cx);
        let terminal = cx.debug_bounds("terminal").unwrap();
        let cells = view.read_with(cx, |view, _| view.bounds);
        assert_eq!(cells.left(), terminal.left() + px(16.));
        assert_eq!(cells.right(), terminal.right() - px(8.));
        assert_eq!(cells.bottom(), terminal.bottom() - px(12.));
        (cx.debug_bounds("flash").unwrap(), cells)
    };

    let (flash, cells) = show(cx, ClipboardToastPosition::TopLeft);
    assert_eq!(flash.left(), cells.left() + px(12.));
    assert_eq!(flash.top(), cells.top() + px(12.));

    let (flash, cells) = show(cx, ClipboardToastPosition::BottomRight);
    assert_eq!(flash.right(), cells.right() - px(12.));
    assert_eq!(flash.bottom(), cells.bottom() - px(12.));
}
