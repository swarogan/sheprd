use super::*;

#[derive(Clone, Copy, Debug)]
enum Event {
    Back,
    Forward,
    Complete(&'static str),
    Reject,
    Release,
}

struct Execution {
    recent: RecentPanes,
    focused: String,
    pending: Option<String>,
    attempts: usize,
    released: bool,
    rejected: bool,
    confirmed: Option<String>,
    description: Vec<String>,
}

fn replay(events: &[Event]) -> Execution {
    let mut execution = Execution {
        recent: visited(&["a", "b", "c"]),
        focused: "c".into(),
        pending: None,
        attempts: 0,
        released: false,
        rejected: false,
        confirmed: None,
        description: Vec::new(),
    };
    for event in events {
        match *event {
            Event::Back | Event::Forward => {
                execution.attempts += 1;
                let step = match event {
                    Event::Back => Step::Back,
                    _ => Step::Forward,
                };
                let candidate = peek_local(&execution.recent, step, &focusing(&execution.focused));
                if execution.pending.is_some() {
                    assert!(
                        candidate.is_none(),
                        "a pending selection must refuse another press"
                    );
                }
                if let Some((index, target)) = candidate {
                    execution.recent.begin(index, Modifiers::control());
                    execution.description.push(format!(
                        "{event:?} queues {target}; snapshot still shows {}",
                        execution.focused
                    ));
                    execution.pending = Some(target);
                } else {
                    execution
                        .description
                        .push(format!("{event:?} is refused while pending"));
                }
            }
            Event::Complete(focused) => {
                let target = execution.pending.take().unwrap();
                execution.description.push(format!(
                    "the fence confirms {target}; the latest coalesced focus is {focused}"
                ));
                execution.confirmed = Some(target);
                execution.focused = focused.into();
                observe_local(&mut execution.recent, &focusing(focused));
            }
            Event::Reject => {
                let target = execution.pending.take().unwrap();
                execution
                    .description
                    .push(format!("reject {target} before it gains focus"));
                execution.recent.cancel();
                execution.rejected = true;
            }
            Event::Release => {
                execution.recent.release(Modifiers::none());
                execution.released = true;
                execution.description.push("release Ctrl".into());
            }
        }
    }
    execution
}

fn check_schedules(max_presses: usize) {
    let mut frontier = VecDeque::from([Vec::new()]);
    let mut completed = 0;
    while let Some(events) = frontier.pop_front() {
        let execution = replay(&events);
        if execution.released && execution.pending.is_none() {
            let mut expected = vec!["c", "b", "a"];
            if let Some(confirmed) = execution.confirmed.as_deref() {
                expected.retain(|pane| *pane != confirmed);
                expected.insert(0, confirmed);
            }
            expected.retain(|pane| *pane != execution.focused);
            expected.insert(0, &execution.focused);
            assert_eq!(
                order(&execution.recent),
                expected,
                "Only confirmed selection and later actual focus may change MRU order.\n{}",
                execution.description.join("\n")
            );
            assert!(!execution.recent.pending());
            completed += 1;
            continue;
        }
        let mut successors = Vec::new();
        if !execution.released {
            if execution.attempts < max_presses && !execution.rejected {
                successors.extend([Event::Back, Event::Forward]);
            }
            if execution.attempts > 0 {
                successors.push(Event::Release);
            }
        }
        if execution.pending.is_some() {
            successors.extend(["a", "b", "c"].into_iter().map(Event::Complete));
            successors.push(Event::Reject);
        }
        for event in successors {
            let mut next = events.clone();
            next.push(event);
            frontier.push_back(next);
        }
    }
    assert!(completed > 0);
    eprintln!("checked {completed} complete schedules, max_presses={max_presses}");
}

#[test]
fn one_press_commits_only_confirmed_focus() {
    check_schedules(1);
}

#[test]
fn bounded_focus_schedules_preserve_mru_order() {
    check_schedules(3);
}
