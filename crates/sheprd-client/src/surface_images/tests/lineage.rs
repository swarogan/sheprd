use super::*;

fn receive(store: &mut ImageStore, mut frame: PaneSurfaceFrame, shown_scene: bool) {
    if shown_scene {
        store.receive_shown_scene(&mut frame.graphics);
    } else {
        store.receive(&mut frame);
        store.show();
    }
    assert!(store.placed.len() <= MAX_PLACEMENTS);
}

#[test]
fn replacements_inherit_only_continuously_placed_matching_sources() {
    for shown_scene in [false, true] {
        for case in [
            "replace",
            "empty",
            "retained",
            "pane",
            "popup",
            "store",
            "size",
            "ambiguous",
        ] {
            let mut store = ImageStore::default();
            let a = key(1, SurfaceGraphicsFormat::Rgba);
            let mut first = placed(&[&a], true);
            if case == "ambiguous" {
                let mut other = a.clone();
                other.data_fingerprint = 99;
                first.graphics.assets.push(asset(&other));
                first.graphics.placements.push(placement(&other));
            }
            receive(&mut store, first, shown_scene);
            let old = store.published().get(&a).unwrap().clone();
            if case == "empty" || case == "retained" {
                let mut gap = SurfaceGraphicsScene::default();
                if case == "retained" {
                    gap.retained_assets.push(a.clone());
                }
                receive(&mut store, surface(gap), shown_scene);
            }
            let mut b = a.clone();
            b.data_fingerprint = 2;
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
                "store" => store = ImageStore::default(),
                "size" => {
                    b.image_width = 1;
                    b.data_len = 8;
                }
                _ => {}
            }
            receive(&mut store, placed(&[&b], true), shown_scene);
            let images = store.published();
            let new = images.get(&b).unwrap();
            assert_ne!(old.serial(), new.serial());
            assert_eq!(old.lineage() == new.lineage(), case == "replace", "{case}");
        }
    }
}

#[test]
fn bytes_delivered_while_only_retained_start_a_fresh_lineage() {
    for shown_scene in [false, true] {
        let mut store = ImageStore::default();
        let a = key(1, SurfaceGraphicsFormat::Rgba);
        receive(&mut store, placed(&[&a], true), shown_scene);
        let old = store.published().get(&a).unwrap().lineage();
        let mut b = a.clone();
        b.data_fingerprint = 2;
        // B's bytes arrive while nothing places that source, then B is placed.
        let gap = SurfaceGraphicsScene {
            assets: vec![asset(&b)],
            placements: vec![],
            retained_assets: vec![b.clone()],
        };
        receive(&mut store, surface(gap), shown_scene);
        receive(&mut store, placed(&[&b], false), shown_scene);
        let new = store.published().get(&b).unwrap().lineage();
        assert_ne!(old, new, "shown_scene={shown_scene}");
    }
}

#[test]
fn collected_images_always_start_fresh_lineages() {
    let a = key(1, SurfaceGraphicsFormat::Rgba);
    let first: SurfaceImages = [asset(&a)].into_iter().collect();
    let next: SurfaceImages = [asset(&a)].into_iter().collect();
    assert_ne!(
        first.get(&a).unwrap().lineage(),
        next.get(&a).unwrap().lineage()
    );
}

fn retained_replacement_placed_again(shown_scene: bool, different_size: bool) {
    let mut store = ImageStore::default();
    let a = key(1, SurfaceGraphicsFormat::Rgba);
    receive(&mut store, placed(&[&a], true), shown_scene);
    let old = store.published().get(&a).unwrap().clone();
    let mut b = a.clone();
    b.data_fingerprint = 2;
    receive(&mut store, placed(&[&b], true), shown_scene);
    let before = store.published().get(&b).unwrap().clone();
    assert_eq!(old.lineage(), before.lineage());
    let mut gap = SurfaceGraphicsScene {
        retained_assets: vec![b.clone()],
        ..Default::default()
    };
    if different_size {
        let mut x = a.clone();
        x.data_fingerprint = 3;
        x.image_width = 1;
        x.data_len = 8;
        gap.assets.push(asset(&x));
        gap.placements.push(placement(&x));
    }
    receive(&mut store, surface(gap), shown_scene);
    let mut again = placed(&[&b], false);
    let changed = if shown_scene {
        store.receive_shown_scene(&mut again.graphics)
    } else {
        let changed = store.receive(&mut again);
        store.show();
        changed
    };
    let after = store.published().get(&b).unwrap().clone();
    assert_eq!(before.serial(), after.serial());
    assert!(Arc::ptr_eq(before.data(), after.data()));
    assert_ne!(
        before.lineage(),
        after.lineage(),
        "shown_scene={shown_scene}"
    );
    assert!(changed, "lineage changes must be published");
}

#[test]
fn a_retained_replacement_placed_after_an_empty_scene_starts_a_new_lineage() {
    for shown_scene in [false, true] {
        retained_replacement_placed_again(shown_scene, false);
    }
}

#[test]
fn a_retained_replacement_placed_after_another_size_starts_a_new_lineage() {
    for shown_scene in [false, true] {
        retained_replacement_placed_again(shown_scene, true);
    }
}

#[test]
fn continuously_placed_retained_images_do_not_churn_lineages() {
    for shown_scene in [false, true] {
        for ambiguous in [false, true] {
            let mut store = ImageStore::default();
            let a = key(1, SurfaceGraphicsFormat::Rgba);
            let mut b = a.clone();
            b.data_fingerprint = 2;
            let keys = if ambiguous { vec![&a, &b] } else { vec![&b] };
            receive(&mut store, placed(&keys, true), shown_scene);
            let before = store.published();
            for _ in 0..3 {
                let mut next = placed(&keys, false);
                let changed = if shown_scene {
                    store.receive_shown_scene(&mut next.graphics)
                } else {
                    let changed = store.receive(&mut next);
                    assert!(!store.show());
                    changed
                };
                assert!(!changed);
                let after = store.published();
                for key in &keys {
                    assert_eq!(
                        before.get(key).unwrap().lineage(),
                        after.get(key).unwrap().lineage()
                    );
                }
            }
        }
    }
}
