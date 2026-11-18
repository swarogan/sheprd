//! A graphics command tears a synchronized update open; the frame the
//! update replaces must still be published whole, even when the same feed
//! ended it and began the next.

use super::{
    graphics::{command, placements},
    *,
};
use crate::Damage;

const PLACED: &str = "a=T,t=d,f=32,s=1,v=1,i=1,p=1,c=1,r=1,C=1";
const BEGIN: &[u8] = b"\x1b[?2026h";
const END: &[u8] = b"\x1b[?2026l";

/// An emulator showing image 1 with everything taken.
fn showing() -> Emulator {
    let mut emulator = emulator(10, 4);
    emulator.feed(&command(PLACED, &[1, 2, 3, 255]));
    assert_eq!(placements(&mut emulator).len(), 1);
    let _ = emulator.take_damage();
    emulator
}

/// A frame writing `text` at the top left and replacing image 1 with
/// `pixel`, sent in two chunks; `last` leaves the second chunk out.
fn frame(text: &str, pixel: [u8; 4], last: bool) -> Vec<u8> {
    let mut bytes = BEGIN.to_vec();
    bytes.extend(format!("\x1b[1;1H{text}").as_bytes());
    bytes.extend(command("a=d,d=i,i=1,p=1", &[]));
    bytes.extend(command(&format!("{PLACED},m=1"), &pixel[..3]));
    if last {
        bytes.extend(command("m=0", &pixel[3..]));
        bytes.extend(END);
    }
    bytes
}

fn top_row(damage: &Damage) -> String {
    let text = |cells: &[CellData]| {
        cells
            .iter()
            .map(|cell| cell.symbol.as_str())
            .collect::<String>()
            .trim_end()
            .to_owned()
    };
    match damage {
        Damage::Full(frame) => line(frame, 0),
        Damage::Rows { rows, .. } => rows
            .iter()
            .find(|row| row.y == 0)
            .map(|row| text(&row.cells))
            .unwrap_or_default(),
    }
}

#[test]
fn a_frame_ended_by_the_feed_that_tears_the_next_is_published() {
    let mut emulator = showing();
    let mut bytes = frame("one", [4, 5, 6, 255], true);
    bytes.extend(frame("two", [9, 8, 7, 255], false));
    emulator.feed(&bytes);
    assert!(emulator.sync_pending());

    assert_eq!(top_row(&emulator.take_damage()), "one");
    let scene = emulator.take_graphics().unwrap();
    assert_eq!(scene.placements.len(), 1);
    assert_eq!(scene.assets[0].data, [4, 5, 6, 255]);
    // Nothing of the torn frame shows until it ends.
    assert!(emulator.take_damage().is_empty());
    assert!(emulator.take_graphics().is_none());

    let mut rest = command("m=0", &[255]);
    rest.extend(END);
    emulator.feed(&rest);
    assert!(!emulator.sync_pending());
    assert_eq!(top_row(&emulator.take_damage()), "two");
    let scene = emulator.take_graphics().unwrap();
    assert_eq!(scene.placements.len(), 1);
    assert_eq!(scene.assets[0].data, [9, 8, 7, 255]);
}

#[test]
fn frames_kept_at_a_tear_and_not_taken_join_the_next_take() {
    let mut emulator = showing();
    let mut bytes = frame("one", [4, 5, 6, 255], true);
    bytes.extend(frame("two", [9, 8, 7, 255], true));
    emulator.feed(&bytes);
    assert!(!emulator.sync_pending());

    let damage = emulator.take_damage();
    assert!(matches!(damage, Damage::Full(_)), "{damage:?}");
    assert_eq!(top_row(&damage), "two");
    let scene = emulator.take_graphics().unwrap();
    assert_eq!(scene.placements.len(), 1);
    assert_eq!(scene.assets.len(), 1);
    assert_eq!(scene.assets[0].data, [9, 8, 7, 255]);
}

#[test]
fn an_image_kept_at_a_tear_still_carries_its_bytes_when_moved() {
    let mut emulator = emulator(10, 4);
    // Frame one shows a new image; frame two, torn, moves it.
    let mut bytes = BEGIN.to_vec();
    bytes.extend(command(PLACED, &[4, 5, 6, 255]));
    bytes.extend(END);
    bytes.extend(BEGIN);
    bytes.extend(b"\x1b[2;3H");
    bytes.extend(command("a=p,i=1,p=1,c=1,r=1,C=1", &[]));
    bytes.extend(END);
    emulator.feed(&bytes);

    let scene = emulator.take_graphics().unwrap();
    assert_eq!(scene.placements.len(), 1);
    assert_eq!((scene.placements[0].x, scene.placements[0].y), (2, 1));
    assert_eq!(scene.assets.len(), 1, "the image was never sent");
    assert_eq!(scene.assets[0].data, [4, 5, 6, 255]);
}

#[test]
fn an_update_ended_and_begun_in_one_feed_counts_as_ended() {
    let mut emulator = emulator(10, 4);
    emulator.feed(BEGIN);
    assert_eq!(emulator.syncs_ended(), 0);
    emulator.feed(b"a\x1b[?2026lb\x1b[?2026hc");
    assert!(emulator.sync_pending());
    assert_eq!(emulator.syncs_ended(), 1);
    // An end split across feeds still counts, torn or not.
    emulator.feed(&command("a=d,d=i,i=1", &[]));
    emulator.feed(b"\x1b[?20");
    assert_eq!(emulator.syncs_ended(), 1);
    emulator.feed(b"26l");
    assert!(!emulator.sync_pending());
    assert_eq!(emulator.syncs_ended(), 2);
    assert_eq!(top_row(&emulator.take_damage()), "abc");
}
