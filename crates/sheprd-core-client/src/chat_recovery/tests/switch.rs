//! A chat that switches agents before its first prompt must not be recovered
//! as the agent and session it switched away from.
use super::*;

#[test]
fn a_switch_holds_a_live_record_as_starting_so_no_sweep_relaunches_the_old_agent() {
    let (core, mut peer) = connected();
    let worker = std::thread::spawn(move || switching(&core, &ID.parse().unwrap(), "t1"));
    let get = peer.request();
    assert_eq!(get["method"], "doc.get");
    peer.answer(&get, json!({"value": record(), "version": 4}));
    let set = peer.request();
    assert_eq!(set["method"], "doc.set");
    assert_eq!(set["params"]["version"], 4);
    assert_eq!(set["params"]["value"]["state"], "starting");
    assert_eq!(set["params"]["value"]["session"], "exact-session");
    peer.answer(&set, json!({"version": 5}));
    worker.join().unwrap().unwrap();
}

#[test]
fn a_switch_before_any_session_opened_or_a_second_switch_writes_nothing() {
    for held in [None, Some(State::Starting)] {
        let (core, mut peer) = connected();
        let worker = std::thread::spawn(move || switching(&core, &ID.parse().unwrap(), "t1"));
        let value = held.map(|state| {
            let mut record = record();
            record.state = state;
            record
        });
        let get = peer.request();
        peer.answer(&get, json!({"value": value, "version": 4}));
        worker.join().unwrap().unwrap();
    }
}

#[test]
fn a_switch_refuses_a_closed_chat_or_a_record_another_terminal_owns() {
    for (state, terminal) in [(State::Closed, "t1"), (State::Live, "t2")] {
        let (core, mut peer) = connected();
        let worker = std::thread::spawn(move || switching(&core, &ID.parse().unwrap(), terminal));
        let mut record = record();
        record.state = state;
        let get = peer.request();
        peer.answer(&get, json!({"value": record, "version": 4}));
        assert!(matches!(
            worker.join().unwrap(),
            Err(crate::Error::ChatRecovery(Error::Closed | Error::Moved))
        ));
    }
}

#[test]
fn the_switched_agents_session_makes_the_held_record_live_again() {
    let (core, mut peer) = connected();
    let worker = std::thread::spawn(move || {
        opened(
            &core,
            &ID.parse().unwrap(),
            "t1",
            Session {
                adapter: false,
                args: Vec::new(),
                agent: "claude".parse().unwrap(),
                child: Identity {
                    core: "c0ffee".into(),
                    terminal: "t1".into(),
                    pid: 60,
                    started: "birth:60".into(),
                },
                cwd: "/work/tree".into(),
                env: BTreeMap::new(),
                program: "/bin/sheprd-acp".into(),
                session: "new-session".into(),
            },
        )
    });
    let list = peer.request();
    assert_eq!(list["method"], "terminal.list");
    peer.answer(&list, json!({"terminals": [terminal("t1")]}));
    let mut held = record();
    held.state = State::Starting;
    let get = peer.request();
    peer.answer(&get, json!({"value": held, "version": 5}));
    let set = peer.request();
    assert_eq!(set["params"]["version"], 5);
    let value = &set["params"]["value"];
    assert_eq!(value["state"], "live");
    assert_eq!(value["agent"], "claude");
    assert_eq!(value["session"], "new-session");
    assert_eq!(value["child"]["pid"], 60);
    peer.answer(&set, json!({"version": 6}));
    worker.join().unwrap().unwrap();
}
