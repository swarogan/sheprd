//! The lines of the helper's scanning side, for adding a nearby Sheprd.

use super::*;

const COMPUTER: &str = "1A2B3C4D-5E6F-7081-92A3-B4C5D6E7F809";

#[test]
fn reads_what_scanning_finds_and_carries() {
    assert_eq!(
        parse_line(r#"{"type":"central_state","state":"powered_off"}"#),
        Some(Event::CentralRadio(Radio::Off))
    );
    let found =
        format!(r#"{{"type":"found","peripheral":"{COMPUTER}","name":"Studio\u0007","fp":"AB"}}"#);
    assert_eq!(
        parse_line(&found),
        Some(Event::Found {
            peripheral: COMPUTER.to_owned(),
            name: "Studio".to_owned()
        })
    );
    let frame = format!(
        r#"{{"type":"peripheral_frame","peripheral":"{COMPUTER}","data":"{}"}}"#,
        STANDARD.encode(b"{}")
    );
    let frame = parse_line(&frame).unwrap();
    assert!(frame.scanned());
    assert_eq!(
        frame,
        Event::PeripheralFrame {
            peripheral: COMPUTER.to_owned(),
            data: b"{}".to_vec()
        }
    );
    assert_eq!(
        parse_line(&format!(
            r#"{{"type":"connected","peripheral":"{COMPUTER}"}}"#
        )),
        Some(Event::Connected {
            peripheral: COMPUTER.to_owned()
        })
    );
    assert!(!Event::Idle(1).scanned());
}

#[test]
fn refuses_what_is_not_a_computer() {
    for line in [
        r#"{"type":"found","peripheral":"not-a-uuid","name":"Studio"}"#.to_owned(),
        format!(r#"{{"type":"found","peripheral":"{COMPUTER}","name":"  "}}"#),
        format!(
            r#"{{"type":"peripheral_frame","peripheral":"{COMPUTER}","data":"{}"}}"#,
            STANDARD.encode(vec![0; MAX_MESSAGE + 1])
        ),
    ] {
        assert_eq!(parse_line(&line), None, "{line}");
    }
}

#[test]
fn writes_the_scanning_commands() {
    // Key order follows serde_json's features, so compare values.
    let value = |line: String| serde_json::from_str::<serde_json::Value>(&line).unwrap();
    assert_eq!(
        value(scan(true)),
        serde_json::json!({ "type": "scan", "on": true })
    );
    assert_eq!(
        value(connect(COMPUTER)),
        serde_json::json!({ "type": "connect", "peripheral": COMPUTER })
    );
    let line: serde_json::Value = serde_json::from_str(&peripheral_frame(COMPUTER, b"hi")).unwrap();
    assert_eq!(line["type"], "peripheral_frame");
    assert_eq!(line["data"], STANDARD.encode(b"hi"));
}
