use super::*;
use crate::{config::Padding, terminal::viewport};
use gpui::{px, size};

/// However tall the window, the last row of cells ends on the terminal's
/// bottom edge, and the daemon is still asked for every row that fits.
#[gpui::test]
fn the_last_row_sits_on_the_bottom_edge_at_any_window_height(cx: &mut gpui::TestAppContext) {
    let (view, cx) = cx.add_window_view(fixture_window);
    assert_rows_end_on_the_bottom_edge(&view, cx);
}

/// Rows a fraction of a pixel tall end between device pixels; the painted
/// last row must still meet the bottom edge.
#[gpui::test]
fn fractional_rows_end_on_the_bottom_edge(cx: &mut gpui::TestAppContext) {
    let (view, cx) = cx.add_window_view(fixture_window);
    cx.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.config.terminal.line_height_multiple = Some(1.4);
            cx.notify();
        });
    });
    let row = view.read_with(cx, |view, _| view.config.terminal.line_height());
    assert!(row.fract() != 0., "{row} is a whole number of pixels");
    assert_rows_end_on_the_bottom_edge(&view, cx);
}

fn assert_rows_end_on_the_bottom_edge(
    view: &gpui::Entity<SheprdWindow>,
    cx: &mut gpui::VisualTestContext,
) {
    let row = view.read_with(cx, |view, _| view.config.terminal.line_height());
    for height in [600., 601., 613.5, 619., 640.] {
        cx.simulate_resize(size(px(900.), px(height)));
        cx.update(|window, cx| {
            window.refresh();
            window.draw(cx).clear(cx);
        });
        let terminal = cx.debug_bounds("terminal").unwrap();
        let (cells, asked) = view.read_with(cx, |view, _| (view.bounds, view.options.surface_size));
        let scale = padding::scale(cx);
        padding::assert_bottom_aligned(cells, terminal, Padding::default(), row, scale);
        assert_eq!(
            asked,
            viewport(
                f32::from(terminal.size.width),
                f32::from(terminal.size.height),
                view.read_with(cx, |view, _| view.cell_width),
                row,
            ),
            "{height}px tall, rows of {row}"
        );
    }
}
