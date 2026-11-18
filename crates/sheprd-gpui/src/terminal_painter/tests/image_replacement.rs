#![allow(clippy::unwrap_used, reason = "test assertions")]

use super::*;
use sheprd_client::{
    ImageStore, SurfaceImages,
    protocol::{
        SurfaceGraphicsAsset, SurfaceGraphicsAssetKey, SurfaceGraphicsFormat,
        SurfaceGraphicsPlacement, SurfaceGraphicsScene, SurfaceGraphicsSource,
        SurfaceGraphicsTarget,
    },
};
use std::{cell::RefCell, rc::Rc};

fn key(fingerprint: u64) -> SurfaceGraphicsAssetKey {
    SurfaceGraphicsAssetKey {
        source: SurfaceGraphicsSource::Terminal {
            target: SurfaceGraphicsTarget::Pane {
                pane_id: "p1".into(),
            },
            image_id: 1,
        },
        image_width: 1,
        image_height: 1,
        format: SurfaceGraphicsFormat::Rgba,
        data_len: 4,
        data_fingerprint: fingerprint,
    }
}

fn scene(key: &SurfaceGraphicsAssetKey) -> SurfaceGraphicsScene {
    SurfaceGraphicsScene {
        assets: vec![SurfaceGraphicsAsset {
            key: key.clone(),
            data: vec![7; key.data_len as usize],
        }],
        placements: vec![SurfaceGraphicsPlacement {
            asset: key.clone(),
            logical_placement_id: 1,
            x: 0,
            y: 0,
            cols: 1,
            rows: 1,
            source_x: 0,
            source_y: 0,
            source_width: 0,
            source_height: 0,
            x_offset: 0,
            y_offset: 0,
            z: 0,
            scrollback_offset: 0,
        }],
        retained_assets: vec![],
    }
}

fn draw(
    cx: &mut VisualTestContext,
    painter: &Rc<RefCell<TerminalPainter>>,
    scene: &SurfaceGraphicsScene,
    images: Arc<SurfaceImages>,
    popup: bool,
) {
    painter.borrow_mut().painted_images.clear();
    let painter = painter.clone();
    let placements = scene.placements.clone();
    let frame = FrameData {
        width: 1,
        height: 1,
        cells: vec![cell(" ")],
        cursor: None,
        hyperlinks: vec![],
        graphics: vec![],
    };
    cx.draw(Point::default(), size(px(800.), px(600.)), |_, _| {
        canvas(
            |_, _, _| (),
            move |_, _, window, cx| {
                painter.borrow_mut().paint_frame(
                    &frame,
                    point(px(10.), px(10.)),
                    None,
                    Ground::Opaque,
                    10.,
                    &font("Menlo"),
                    &[],
                    &[],
                    None,
                    Some(PlacedImages {
                        placements: &placements,
                        images: &images,
                        target: if popup {
                            ImageTarget::Popup("t")
                        } else {
                            ImageTarget::Main
                        },
                    }),
                    window,
                    cx,
                );
            },
        )
        .size_full()
    });
}

fn painter() -> Rc<RefCell<TerminalPainter>> {
    let painter = Rc::new(RefCell::new(TerminalPainter::default()));
    painter
        .borrow_mut()
        .set_appearance(14., 20., Theme::default());
    painter
}

fn assert_image(painter: &Rc<RefCell<TerminalPainter>>) {
    assert_eq!(
        painter.borrow().painted_images,
        [(
            0,
            Bounds::new(point(px(10.), px(10.)), size(px(10.), px(20.)))
        )]
    );
}

#[gpui::test]
fn replacement_keeps_the_last_decoded_image(cx: &mut TestAppContext) {
    let (_, cx) = cx.add_window_view(|_, _| Empty);
    let painter = painter();
    let mut store = ImageStore::default();
    let a = key(1);
    let mut first = scene(&a);
    store.receive_shown_scene(&mut first);
    let serial = store.published().get(&a).unwrap().serial();
    draw(cx, &painter, &first, store.published(), false);
    assert!(painter.borrow().painted_images.is_empty());
    cx.run_until_parked();
    draw(cx, &painter, &first, store.published(), false);
    assert_image(&painter);
    let b = key(2);
    let mut next = scene(&b);
    store.receive_shown_scene(&mut next);
    assert_ne!(store.published().get(&b).unwrap().serial(), serial);
    assert!(store.published().get(&a).is_none());
    draw(cx, &painter, &next, store.published(), false);
    assert_image(&painter);
    cx.run_until_parked();
    draw(cx, &painter, &next, store.published(), false);
    assert_image(&painter);
}

#[gpui::test]
fn waiting_for_a_decode_slot_keeps_the_last_image(cx: &mut TestAppContext) {
    let (_, cx) = cx.add_window_view(|_, _| Empty);
    let painter = painter();
    let mut store = ImageStore::default();
    let mut first = scene(&key(1));
    store.receive_shown_scene(&mut first);
    draw(cx, &painter, &first, store.published(), false);
    cx.run_until_parked();
    draw(cx, &painter, &first, store.published(), false);
    assert_image(&painter);
    // Two replacements start decodes, but neither has run when the third arrives.
    for fingerprint in 2..=4 {
        let mut next = scene(&key(fingerprint));
        store.receive_shown_scene(&mut next);
        draw(cx, &painter, &next, store.published(), false);
    }
    assert_image(&painter);
}

#[gpui::test]
fn discontinuities_never_reuse_an_old_texture(cx: &mut TestAppContext) {
    let (_, cx) = cx.add_window_view(|_, _| Empty);
    for case in [
        "empty", "pane", "popup", "store", "size", "failed", "invalid",
    ] {
        let painter = painter();
        let mut store = ImageStore::default();
        let mut a = key(1);
        if case == "failed" {
            a.format = SurfaceGraphicsFormat::Png;
        }
        let mut first = scene(&a);
        store.receive_shown_scene(&mut first);
        draw(cx, &painter, &first, store.published(), false);
        cx.run_until_parked();
        draw(cx, &painter, &first, store.published(), false);
        if case == "failed" {
            assert!(painter.borrow().painted_images.is_empty());
        } else {
            assert_image(&painter);
        }
        if case == "empty" {
            let mut empty = SurfaceGraphicsScene::default();
            store.receive_shown_scene(&mut empty);
            draw(cx, &painter, &empty, store.published(), false);
            assert!(painter.borrow().painted_images.is_empty());
        }
        let mut b = key(2);
        match case {
            "pane" => {
                b.source = SurfaceGraphicsSource::Terminal {
                    target: SurfaceGraphicsTarget::Pane {
                        pane_id: "p2".into(),
                    },
                    image_id: 1,
                }
            }
            "popup" => {
                b.source = SurfaceGraphicsSource::Terminal {
                    target: SurfaceGraphicsTarget::Popup {
                        terminal_id: "t".into(),
                    },
                    image_id: 1,
                }
            }
            "store" => {
                store = ImageStore::default();
                b = a.clone();
            }
            "size" => {
                b.image_width = 2;
                b.data_len = 8;
            }
            _ => {}
        }
        let mut next = scene(&b);
        if case == "invalid" {
            next.assets[0].data.pop();
        }
        store.receive_shown_scene(&mut next);
        draw(cx, &painter, &next, store.published(), case == "popup");
        assert!(painter.borrow().painted_images.is_empty(), "{case}");
        cx.run_until_parked();
    }
}
