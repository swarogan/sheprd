//! The width of the rail beside an agent chat is remembered with the rest of
//! the chrome, and a reset forgets it.
use super::*;
use gpui::{Bounds, TestAppContext, point, px, size};
// Over the glob's GPUI `test`, which `#[gpui::test]` would expand into.
use core::prelude::v1::test;

/// Waits for the worker to write a file whose rail width is `width`.
fn await_saved(path: &Path, width: Option<f32>) -> Chrome {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(chrome) = read_chrome(path)
            .ok()
            .filter(|chrome| chrome.chat_rail_width == width)
        {
            return chrome;
        }
        assert!(Instant::now() < deadline, "rail width {width:?} not saved");
        thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn the_rail_width_round_trips_and_a_damaged_one_is_forgotten() {
    let directory = TestDirectory::new();
    let path = directory.0.join("preferences.json");
    let chrome = Chrome {
        chat_rail_width: Some(360.),
        ..Chrome::default()
    };
    write_chrome(&path, chrome.clone()).unwrap();
    assert_eq!(read_chrome(&path).unwrap(), chrome);
    // A bad width loses only itself, never the sidebar's.
    for damaged in [r#""wide""#, "-5", "0", "null"] {
        fs::write(
            &path,
            format!(r#"{{"sidebar_width_px": 250, "chat_rail_width_px": {damaged}}}"#),
        )
        .unwrap();
        let read = read_chrome(&path).unwrap();
        assert_eq!(read.chat_rail_width, None, "{damaged}");
        assert_eq!(read.sidebar_width, Some(250.), "{damaged}");
    }
}

#[gpui::test]
fn a_dragged_rail_width_survives_a_restart_and_a_reset_forgets_it(cx: &mut TestAppContext) {
    let directory = TestDirectory::new();
    let path = directory.0.join("preferences.json");
    let (view, cx) = cx.add_window_view(crate::sidebar::layout_tests::fixture_window);
    // The rail laid out 240px wide at the window's right, dragged 120px left.
    let rail = Bounds::new(point(px(760.), px(0.)), size(px(240.), px(600.)));
    view.update(cx, |view, _| {
        let mut preferences = Preferences::start(Ok(path.clone()));
        await_loaded(&mut preferences);
        view.sidebar_preferences = Some(preferences);
        assert!(
            view.chat_rail_width
                .drag(crate::panel_resize::Side::Right, rail, px(640.))
        );
        view.save_chrome();
    });
    let saved = await_saved(&path, Some(360.));

    // A window opened after a restart takes the saved width.
    let (restarted, cx) = cx.add_window_view(crate::sidebar::layout_tests::fixture_window);
    restarted.update(cx, |view, _| {
        view.apply_stored_chrome(saved);
        assert_eq!(view.chat_rail_width.chosen(), Some(360.));
        assert_eq!(view.chat_rail_width.width(0.), 360.);
    });

    // A double-click's reset is saved, and the next restart is at the default.
    view.update(cx, |view, _| {
        view.chat_rail_width.reset();
        view.save_chrome();
    });
    let reset = await_saved(&path, None);
    let (again, cx) = cx.add_window_view(crate::sidebar::layout_tests::fixture_window);
    again.update(cx, |view, _| {
        view.apply_stored_chrome(reset);
        assert_eq!(view.chat_rail_width.chosen(), None);
        assert_eq!(
            view.chat_rail_width.width(0.),
            crate::panel_resize::CHAT_RAIL.width(0.)
        );
    });
}
