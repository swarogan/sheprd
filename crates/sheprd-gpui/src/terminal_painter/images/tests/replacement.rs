use super::*;
use sheprd_client::{
    ImageStore,
    protocol::{SurfaceGraphicsAsset, SurfaceGraphicsScene},
};

fn replacement(store: &mut ImageStore, fingerprint: u64) -> SurfaceImage {
    let mut key = key(SurfaceGraphicsFormat::Rgba, 1, 1);
    key.data_fingerprint = fingerprint;
    let mut scene = SurfaceGraphicsScene {
        assets: vec![SurfaceGraphicsAsset {
            key: key.clone(),
            data: vec![7; 4],
        }],
        placements: vec![placement(key.clone())],
        retained_assets: vec![],
    };
    store.receive_shown_scene(&mut scene);
    store.published().get(&key).unwrap().clone()
}

fn paint(cache: &mut ImageCache, image: &SurfaceImage, now: Instant) -> Option<Arc<RenderImage>> {
    let lookup = cache.lookup(image.serial(), now);
    cache.for_paint(image, lookup, now)
}

#[test]
fn failed_decode_clears_the_fallback_for_subsequent_replacements() {
    let (mut cache, mut store) = (ImageCache::default(), ImageStore::default());
    let now = Instant::now();
    let a = replacement(&mut store, 1);
    ready(&mut cache, a.serial(), 1, now);
    let old = paint(&mut cache, &a, now).unwrap();
    let b = replacement(&mut store, 2);
    assert!(Arc::ptr_eq(&old, &paint(&mut cache, &b, now).unwrap()));
    cache.finish(b.serial(), Err(Error::PaneImageLimit));
    assert!(paint(&mut cache, &b, now).is_none());
    assert_eq!(cache.shown.get(&b.lineage()), Some(&b.serial()));
    let c = replacement(&mut store, 3);
    assert!(paint(&mut cache, &c, now).is_none());
}

#[test]
fn evicted_textures_cannot_be_used_as_fallbacks() {
    let (mut cache, mut store) = (ImageCache::default(), ImageStore::default());
    let now = Instant::now();
    let a = replacement(&mut store, 1);
    ready(&mut cache, a.serial(), 1, now);
    assert!(paint(&mut cache, &a, now).is_some());
    assert_eq!(cache.begin(now + IDLE).len(), 1);
    assert!(cache.shown.is_empty());
    let b = replacement(&mut store, 2);
    assert!(paint(&mut cache, &b, now + IDLE).is_none());
}

#[test]
fn fallback_is_used_this_paint_and_late_completion_takes_over() {
    let (mut cache, mut store) = (ImageCache::default(), ImageStore::default());
    let now = Instant::now();
    let a = replacement(&mut store, 1);
    ready(&mut cache, a.serial(), 1, now);
    let old = paint(&mut cache, &a, now).unwrap();
    let b = replacement(&mut store, 2);
    assert!(Arc::ptr_eq(&old, &paint(&mut cache, &b, now).unwrap()));
    let c = replacement(&mut store, 3);
    // The scene moved on to C before B finished: B still becomes the newest
    // ready frame, or a stream faster than the decoder would freeze on A.
    let finished = Arc::new(texture(RgbaImage::new(1, 1)));
    cache.finish(b.serial(), Ok(finished.clone()));
    let later = now + IDLE / 2;
    cache.begin(later);
    assert!(Arc::ptr_eq(
        &finished,
        &paint(&mut cache, &c, later).unwrap()
    ));
}

#[test]
fn a_slow_older_decode_never_moves_the_fallback_back() {
    let (mut cache, mut store) = (ImageCache::default(), ImageStore::default());
    let now = Instant::now();
    let a = replacement(&mut store, 1);
    ready(&mut cache, a.serial(), 1, now);
    assert!(paint(&mut cache, &a, now).is_some());
    let (older, newer) = (replacement(&mut store, 2), replacement(&mut store, 3));
    assert!(paint(&mut cache, &older, now).is_some());
    assert!(paint(&mut cache, &newer, now).is_some());
    assert!(cache.pending.len() <= MAX_DECODES);
    let newest = Arc::new(texture(RgbaImage::new(1, 1)));
    cache.finish(newer.serial(), Ok(newest.clone()));
    cache.finish(older.serial(), Ok(Arc::new(texture(RgbaImage::new(1, 1)))));
    assert!(cache.pending.is_empty());
    let next = replacement(&mut store, 4);
    assert!(Arc::ptr_eq(
        &newest,
        &paint(&mut cache, &next, now).unwrap()
    ));
}

#[test]
fn shown_mappings_are_bounded_by_cache_entries() {
    let mut cache = ImageCache::default();
    let start = Instant::now();
    for index in 0..MAX_ENTRIES * 2 {
        let now = start + IDLE * u32::try_from(index).unwrap();
        cache.begin(now);
        let mut store = ImageStore::default();
        let image = replacement(&mut store, 1);
        ready(&mut cache, image.serial(), 1, now);
        assert!(paint(&mut cache, &image, now).is_some());
        assert!(cache.shown.len() <= cache.entries.len());
        assert!(cache.shown.len() <= MAX_ENTRIES);
    }
}

#[test]
fn a_stream_of_replacements_keeps_the_cache_small_and_moving() {
    let (mut cache, mut store) = (ImageCache::default(), ImageStore::default());
    let now = Instant::now();
    let first = replacement(&mut store, 1);
    ready(&mut cache, first.serial(), 1, now);
    assert!(paint(&mut cache, &first, now).is_some());
    let mut last = None;
    for fingerprint in 2..400 {
        let frame = replacement(&mut store, fingerprint);
        cache.begin(now);
        let shown = paint(&mut cache, &frame, now).unwrap();
        // The decode of this frame finishes before the next one arrives.
        let fresh = Arc::new(texture(RgbaImage::new(1, 1)));
        cache.finish(frame.serial(), Ok(fresh.clone()));
        cache.begin(now);
        // While this frame decodes, the previous finished frame is shown.
        if let Some(last) = &last {
            assert!(Arc::ptr_eq(&shown, last));
        }
        assert!(
            cache.entries.len() <= MAX_DECODES + 2,
            "superseded frames must not pile up: {} entries",
            cache.entries.len()
        );
        last = Some(fresh);
    }
    // The newest finished frame is the one the next paint falls back to.
    let next = replacement(&mut store, 400);
    assert!(Arc::ptr_eq(
        &last.unwrap(),
        &paint(&mut cache, &next, now).unwrap()
    ));
}

#[test]
fn an_older_frame_placed_again_is_shown_and_not_decoded_forever() {
    let (mut cache, mut store) = (ImageCache::default(), ImageStore::default());
    let now = Instant::now();
    let a = replacement(&mut store, 1);
    ready(&mut cache, a.serial(), 1, now);
    assert!(paint(&mut cache, &a, now).is_some());
    let b = replacement(&mut store, 2);
    assert_eq!(a.lineage(), b.lineage());
    assert!(paint(&mut cache, &b, now).is_some());
    cache.finish(b.serial(), Ok(Arc::new(texture(RgbaImage::new(1, 1)))));
    cache.begin(now);
    assert!(paint(&mut cache, &b, now).is_some());
    cache.begin(now);
    // The scene places A again: its own pixels must come back and stay.
    assert!(paint(&mut cache, &a, now).is_some());
    let own = Arc::new(texture(RgbaImage::new(1, 1)));
    cache.finish(a.serial(), Ok(own.clone()));
    cache.begin(now);
    for _ in 0..3 {
        assert!(matches!(cache.lookup(a.serial(), now), Lookup::Ready(_)));
        assert!(Arc::ptr_eq(&own, &paint(&mut cache, &a, now).unwrap()));
        cache.begin(now);
    }
}

#[test]
fn an_older_frame_placed_again_survives_two_paints_per_frame() {
    let (mut cache, mut store) = (ImageCache::default(), ImageStore::default());
    let now = Instant::now();
    let a = replacement(&mut store, 1);
    ready(&mut cache, a.serial(), 1, now);
    assert!(paint(&mut cache, &a, now).is_some());
    let b = replacement(&mut store, 2);
    assert!(paint(&mut cache, &b, now).is_some());
    cache
        .finished
        .lock()
        .unwrap()
        .push((b.serial(), Ok(RgbaImage::new(1, 1))));
    cache.begin(now);
    assert!(paint(&mut cache, &b, now).is_some());
    cache.begin(now); // the popup's paint in the same frame
    cache.begin(now);
    // The scene places A again; a popup is open, so each frame begins twice.
    assert!(paint(&mut cache, &a, now).is_some());
    cache.begin(now);
    cache
        .finished
        .lock()
        .unwrap()
        .push((a.serial(), Ok(RgbaImage::new(1, 1))));
    cache.begin(now);
    let Lookup::Ready(own) = cache.lookup(a.serial(), now) else {
        panic!("A's own decode was released");
    };
    for _ in 0..3 {
        assert!(Arc::ptr_eq(&own, &paint(&mut cache, &a, now).unwrap()));
        cache.begin(now);
        cache.begin(now);
    }
}

#[test]
fn failure_after_the_scene_moved_on_clears_the_fallback() {
    let (mut cache, mut store) = (ImageCache::default(), ImageStore::default());
    let now = Instant::now();
    let a = replacement(&mut store, 1);
    ready(&mut cache, a.serial(), 1, now);
    let old = paint(&mut cache, &a, now).unwrap();
    let b = replacement(&mut store, 2);
    assert!(Arc::ptr_eq(&old, &paint(&mut cache, &b, now).unwrap()));
    let c = replacement(&mut store, 3);
    assert!(Arc::ptr_eq(&old, &paint(&mut cache, &c, now).unwrap()));
    cache.finish(b.serial(), Err(Error::PaneImageLimit));
    cache.begin(now);
    assert!(paint(&mut cache, &c, now).is_none());
}

#[test]
fn failure_never_lets_an_older_completion_restore() {
    let (mut cache, mut store) = (ImageCache::default(), ImageStore::default());
    let now = Instant::now();
    let a = replacement(&mut store, 1);
    ready(&mut cache, a.serial(), 1, now);
    assert!(paint(&mut cache, &a, now).is_some());
    let b = replacement(&mut store, 2);
    let c = replacement(&mut store, 3);
    assert!(paint(&mut cache, &b, now).is_some());
    assert!(paint(&mut cache, &c, now).is_some());
    cache.finish(c.serial(), Err(Error::PaneImageLimit));
    assert!(paint(&mut cache, &c, now).is_none());
    cache.finish(b.serial(), Ok(Arc::new(texture(RgbaImage::new(1, 1)))));
    cache.begin(now);
    let d = replacement(&mut store, 4);
    assert!(paint(&mut cache, &d, now).is_none());
}

#[test]
fn failure_keeps_a_newer_success() {
    let (mut cache, mut store) = (ImageCache::default(), ImageStore::default());
    let now = Instant::now();
    let a = replacement(&mut store, 1);
    ready(&mut cache, a.serial(), 1, now);
    assert!(paint(&mut cache, &a, now).is_some());
    let b = replacement(&mut store, 2);
    let c = replacement(&mut store, 3);
    assert!(paint(&mut cache, &b, now).is_some());
    assert!(paint(&mut cache, &c, now).is_some());
    let newest = Arc::new(texture(RgbaImage::new(1, 1)));
    cache.finish(c.serial(), Ok(newest.clone()));
    cache.finish(b.serial(), Err(Error::PaneImageLimit));
    let d = replacement(&mut store, 4);
    assert!(Arc::ptr_eq(&newest, &paint(&mut cache, &d, now).unwrap()));
}

fn scene_images(
    store: &mut ImageStore,
    placed: &[u64],
    retained: &[u64],
    with_bytes: bool,
) -> Arc<SurfaceImages> {
    let key_for = |fingerprint| {
        let mut key = key(SurfaceGraphicsFormat::Rgba, 1, 1);
        key.data_fingerprint = fingerprint;
        key
    };
    let mut scene = SurfaceGraphicsScene {
        assets: if with_bytes {
            placed
                .iter()
                .map(|id| SurfaceGraphicsAsset {
                    key: key_for(*id),
                    data: vec![7; 4],
                })
                .collect()
        } else {
            vec![]
        },
        placements: placed.iter().map(|id| placement(key_for(*id))).collect(),
        retained_assets: retained.iter().map(|id| key_for(*id)).collect(),
    };
    store.receive_shown_scene(&mut scene);
    store.published()
}

fn placed_again_during_decode(paints_per_frame: usize) {
    let (mut cache, mut store) = (ImageCache::default(), ImageStore::default());
    let now = Instant::now();
    let a = replacement(&mut store, 1);
    ready(&mut cache, a.serial(), 1, now);
    let begin_frame = |cache: &mut ImageCache| {
        for _ in 0..paints_per_frame {
            cache.begin(now);
        }
    };
    begin_frame(&mut cache);
    let own = paint(&mut cache, &a, now).unwrap();
    let images = scene_images(&mut store, &[2], &[1], true);
    let mut b_key = key(SurfaceGraphicsFormat::Rgba, 1, 1);
    b_key.data_fingerprint = 2;
    let b = images.get(&b_key).unwrap();
    assert_eq!(a.lineage(), b.lineage());
    begin_frame(&mut cache);
    assert!(Arc::ptr_eq(&own, &paint(&mut cache, b, now).unwrap()));
    scene_images(&mut store, &[1], &[2], false);
    begin_frame(&mut cache);
    assert!(Arc::ptr_eq(&own, &paint(&mut cache, &a, now).unwrap()));
    cache.finish(b.serial(), Ok(Arc::new(texture(RgbaImage::new(1, 1)))));
    begin_frame(&mut cache);
    for _ in 0..3 {
        assert!(cache.entries.contains_key(&a.serial()));
        assert!(matches!(cache.lookup(a.serial(), now), Lookup::Ready(_)));
        assert!(Arc::ptr_eq(&own, &paint(&mut cache, &a, now).unwrap()));
        begin_frame(&mut cache);
    }
}

#[test]
fn an_older_frame_placed_again_while_a_newer_decodes_keeps_its_texture() {
    placed_again_during_decode(1);
}

#[test]
fn an_older_frame_placed_again_while_a_newer_decodes_survives_two_paints() {
    placed_again_during_decode(2);
}

#[test]
fn late_completion_takes_over_with_paints_between() {
    let (mut cache, mut store) = (ImageCache::default(), ImageStore::default());
    let now = Instant::now();
    let a = replacement(&mut store, 1);
    ready(&mut cache, a.serial(), 1, now);
    cache.begin(now);
    let old = paint(&mut cache, &a, now).unwrap();
    let b = replacement(&mut store, 2);
    cache.begin(now);
    assert!(Arc::ptr_eq(&old, &paint(&mut cache, &b, now).unwrap()));
    let c = replacement(&mut store, 3);
    cache.begin(now);
    assert!(Arc::ptr_eq(&old, &paint(&mut cache, &c, now).unwrap()));
    let fresh = Arc::new(texture(RgbaImage::new(1, 1)));
    cache.finish(b.serial(), Ok(fresh.clone()));
    cache.begin(now);
    assert!(Arc::ptr_eq(&fresh, &paint(&mut cache, &c, now).unwrap()));
}

#[test]
fn a_retained_pending_replacement_after_a_gap_has_no_fallback() {
    for with_bytes in [false, true] {
        let (mut cache, mut store) = (ImageCache::default(), ImageStore::default());
        let now = Instant::now();
        let a = replacement(&mut store, 1);
        ready(&mut cache, a.serial(), 1, now);
        let old = paint(&mut cache, &a, now).unwrap();
        let b = replacement(&mut store, 2);
        assert!(Arc::ptr_eq(&old, &paint(&mut cache, &b, now).unwrap()));
        scene_images(&mut store, &[], &[2], false);
        let images = scene_images(&mut store, &[2], &[], with_bytes);
        let mut b_key = key(SurfaceGraphicsFormat::Rgba, 1, 1);
        b_key.data_fingerprint = 2;
        let again = images.get(&b_key).unwrap();
        assert_eq!(b.serial(), again.serial());
        assert!(paint(&mut cache, again, now).is_none());
        let fresh = Arc::new(texture(RgbaImage::new(1, 1)));
        cache.finish(b.serial(), Ok(fresh.clone()));
        cache.begin(now);
        assert!(Arc::ptr_eq(&fresh, &paint(&mut cache, again, now).unwrap()));
    }
}

#[test]
fn retained_images_with_new_lineages_keep_shown_mappings_bounded() {
    let (mut cache, mut store) = (ImageCache::default(), ImageStore::default());
    let now = Instant::now();
    let first = replacement(&mut store, 1);
    ready(&mut cache, first.serial(), 1, now);
    assert!(paint(&mut cache, &first, now).is_some());
    let mut image_key = key(SurfaceGraphicsFormat::Rgba, 1, 1);
    image_key.data_fingerprint = 1;
    for _ in 0..2 * MAX_ENTRIES {
        scene_images(&mut store, &[], &[1], false);
        let images = scene_images(&mut store, &[1], &[], false);
        let again = images.get(&image_key).unwrap();
        cache.begin(now);
        assert!(paint(&mut cache, again, now).is_some());
        assert!(cache.shown.len() <= cache.entries.len());
    }
}

#[test]
fn a_waiting_decode_refreshes_its_pending_lineage_after_a_retained_gap() {
    let (mut cache, mut store) = (ImageCache::default(), ImageStore::default());
    let now = Instant::now();
    let a = replacement(&mut store, 1);
    ready(&mut cache, a.serial(), 1, now);
    let old = paint(&mut cache, &a, now).unwrap();
    let b = replacement(&mut store, 2);
    assert!(Arc::ptr_eq(&old, &paint(&mut cache, &b, now).unwrap()));
    assert_eq!(cache.pending.get(&b.serial()), Some(&b.lineage()));

    scene_images(&mut store, &[], &[2], false);
    let images = scene_images(&mut store, &[2], &[], false);
    let mut b_key = key(SurfaceGraphicsFormat::Rgba, 1, 1);
    b_key.data_fingerprint = 2;
    let again = images.get(&b_key).unwrap();
    assert_eq!(b.serial(), again.serial());
    assert!(Arc::ptr_eq(b.data(), again.data()));
    assert_ne!(b.lineage(), again.lineage());
    let lookup = cache.lookup(again.serial(), now);
    assert!(matches!(lookup, Lookup::Wait));
    // Refresh the decode's handoff even though this lineage has no fallback.
    assert!(cache.for_paint(again, lookup, now).is_none());

    let c = replacement(&mut store, 3);
    assert_eq!(again.lineage(), c.lineage());
    let lookup = cache.lookup(c.serial(), now);
    assert!(matches!(lookup, Lookup::Decode));
    assert!(cache.for_paint(&c, lookup, now).is_none());
    let fresh = Arc::new(texture(RgbaImage::new(1, 1)));
    cache.finish(b.serial(), Ok(fresh.clone()));
    cache.begin(now);
    assert!(paint(&mut cache, &c, now).is_some_and(|shown| Arc::ptr_eq(&shown, &fresh)));
}

#[test]
fn waiting_for_capacity_does_not_create_a_pending_lineage() {
    let (mut cache, mut store) = (ImageCache::default(), ImageStore::default());
    let now = Instant::now();
    for fingerprint in 0..u64::try_from(MAX_DECODES).unwrap() {
        let image = replacement(&mut store, fingerprint);
        assert!(paint(&mut cache, &image, now).is_none());
    }
    let blocked = replacement(&mut store, u64::MAX);
    let lookup = cache.lookup(blocked.serial(), now);
    assert!(matches!(lookup, Lookup::Wait));
    assert!(cache.for_paint(&blocked, lookup, now).is_none());
    assert!(!cache.pending.contains_key(&blocked.serial()));
    assert_eq!(cache.pending.len(), MAX_DECODES);
    assert_eq!(cache.decoding, MAX_DECODES);
}

#[test]
fn an_older_frame_placed_again_is_the_next_fallback() {
    let (mut cache, mut store) = (ImageCache::default(), ImageStore::default());
    let now = Instant::now();
    let key_for = |fingerprint| {
        let mut key = key(SurfaceGraphicsFormat::Rgba, 1, 1);
        key.data_fingerprint = fingerprint;
        key
    };
    let a = replacement(&mut store, 1);
    ready(&mut cache, a.serial(), 1, now);
    cache.begin(now);
    assert!(paint(&mut cache, &a, now).is_some());
    // B replaces A, which the scene keeps.
    let images = scene_images(&mut store, &[2], &[1], true);
    let b = images.get(&key_for(2)).unwrap().clone();
    assert_eq!(a.lineage(), b.lineage());
    ready(&mut cache, b.serial(), 1, now);
    cache.begin(now);
    let b_texture = paint(&mut cache, &b, now).unwrap();
    // A is placed again, B kept; A's own pixels come back.
    let images = scene_images(&mut store, &[1], &[2], false);
    let again = images.get(&key_for(1)).unwrap().clone();
    let mut a_texture = None;
    for _ in 0..3 {
        cache.begin(now);
        if let Lookup::Ready(_) = cache.lookup(again.serial(), now) {
            a_texture = paint(&mut cache, &again, now);
            break;
        }
        let _ = paint(&mut cache, &again, now);
        cache.finish(again.serial(), Ok(Arc::new(texture(RgbaImage::new(1, 1)))));
    }
    let a_texture = a_texture.unwrap();
    assert!(!Arc::ptr_eq(&a_texture, &b_texture));
    // C replaces A: while C decodes, A shows, never B.
    let images = scene_images(&mut store, &[3], &[], true);
    let c = images.get(&key_for(3)).unwrap().clone();
    cache.begin(now);
    let shown = paint(&mut cache, &c, now).unwrap();
    assert!(Arc::ptr_eq(&shown, &a_texture), "C fell back to B");
}
