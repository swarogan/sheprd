//! Scanning shared by every window's Add Device: each live scanner sees
//! every event, and one going leaves the others theirs.

#![allow(clippy::unwrap_used)]

use super::*;
use gpui::TestAppContext;

fn found(name: &str) -> Event {
    Event::Found {
        peripheral: "1A2B3C4D-5E6F-7081-92A3-B4C5D6E7F809".into(),
        name: name.into(),
    }
}

#[gpui::test]
fn every_window_scanning_sees_what_is_found(cx: &mut TestAppContext) {
    cx.update(|cx| {
        Station::install(cx);
        let first = Station::scan(cx).unwrap();
        let second = Station::scan(cx).unwrap();
        let generation = cx.global::<Station>().generation;
        Station::scanned(cx, generation, found("Studio"));
        assert_eq!(first.events.try_recv().unwrap(), found("Studio"));
        assert_eq!(second.events.try_recv().unwrap(), found("Studio"));

        // The first window closes: the second still hears, and scanning
        // goes on for it.
        drop(first);
        assert!(cx.global::<Station>().wanted());
        Station::scanned(cx, generation, found("Laptop"));
        assert_eq!(second.events.try_recv().unwrap(), found("Laptop"));
        drop(second);
        assert!(!cx.global::<Station>().wanted());
    });
}
