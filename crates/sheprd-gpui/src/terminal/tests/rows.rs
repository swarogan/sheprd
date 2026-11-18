use super::*;
use crate::terminal::rows::{bottom_aligned, whole_rows};

/// On whole device pixels at every scale tested, as GPUI lays boxes out.
fn within(height: f32) -> Bounds<Pixels> {
    Bounds::new(point(px(4.), px(8.)), size(px(800.), px(height)))
}

/// The device pixel `logical` lands on, rounded half toward zero as GPUI
/// snaps layout and the painter snaps row edges.
fn device_pixel(logical: Pixels, scale: f32) -> f32 {
    let device = f32::from(logical) * scale;
    (device.abs() - 0.5).ceil().copysign(device)
}

/// Where the painter puts the bottom edge of the last of `rows` rows drawn
/// from the top of `grid`.
fn painted_bottom(grid: Bounds<Pixels>, rows: f32, row: f32, scale: f32) -> f32 {
    device_pixel(grid.top() + px(rows * row), scale)
}

/// The painted last row ends on `edge`. Rows that end exactly half a device
/// pixel past it may round one further, which the clip at the edge hides;
/// they must never round short of it.
fn assert_ends_on(
    grid: Bounds<Pixels>,
    rows: f32,
    row: f32,
    scale: f32,
    edge: Pixels,
    context: &str,
) {
    let painted = painted_bottom(grid, rows, row, scale);
    let edge = device_pixel(edge, scale);
    let past = f32::from(grid.top() + px(rows * row)) * scale - edge;
    let tie = (past - 0.5).abs() < 1e-3;
    assert!(
        painted == edge || (tie && painted == edge + 1.),
        "{context}: painted to {painted}, edge at {edge}"
    );
}

#[test]
fn the_remainder_moves_above_the_first_row() {
    let outer = within(519.);
    let grid = bottom_aligned(outer, 20., 1.);
    assert_eq!(grid.bottom(), outer.bottom());
    assert_eq!(grid.size.height, px(500.));
    assert_eq!((grid.left(), grid.right()), (outer.left(), outer.right()));
}

#[test]
fn a_box_of_whole_rows_and_one_under_a_row_are_left_alone() {
    let exact = within(480.);
    assert_eq!(bottom_aligned(exact, 20., 1.), exact);
    let short = within(12.);
    assert_eq!(bottom_aligned(short, 20., 1.), short);
}

/// Rows a fraction of a pixel tall end between device pixels. The painted
/// last row, not only the box, must end on the bottom edge: 27 rows of 19.6
/// in 540 pixels drawn from a box ending on that edge stop 0.8 short of it,
/// a pixel short once rounded, which leaves a strip under a prompt separator.
#[test]
fn the_painted_last_row_ends_on_the_bottom_edge_with_fractional_rows() {
    let outer = within(540.);
    let grid = bottom_aligned(outer, 19.6, 1.);
    assert_eq!(painted_bottom(grid, 27., 19.6, 1.), 548.);
    assert_eq!(
        viewport(800., f32::from(grid.size.height), 10., 19.6).rows,
        27
    );
}

/// A fractional row height and display scale must never cost the daemon a
/// row, and the last painted row must still end on the bottom edge.
#[test]
fn the_rows_fitted_are_the_rows_asked_for() {
    for scale in [1., 1.25, 1.5, 2.] {
        for row in [18.57, 19.5, 19.6, 20., 21.4, 23.2, 30.] {
            for device in 400..1200 {
                let outer = within(device as f32 / scale);
                let grid = bottom_aligned(outer, row, scale);
                let rows = whole_rows(f32::from(outer.size.height), row);
                let context = format!("{device}px at {scale}x, rows of {row}");
                assert_ends_on(grid, rows, row, scale, outer.bottom(), &context);
                assert_eq!(
                    viewport(800., f32::from(grid.size.height), 10., row).rows,
                    rows as u16,
                    "{context}"
                );
                let overhang = (grid.bottom() - outer.bottom()) * scale;
                assert!(
                    overhang > px(-1e-3) && overhang < px(1. + 1e-3),
                    "{context}: {overhang:?}"
                );
                assert!(grid.top() >= outer.top() - px(1e-3), "{context}");
                assert!(grid.top() - outer.top() < px(row + 1. / scale), "{context}");
                let device_height = f32::from(grid.size.height) * scale;
                assert!(
                    (device_height - device_height.round()).abs() < 1e-3,
                    "{context}"
                );
            }
        }
    }
}
