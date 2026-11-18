use super::*;

#[test]
fn choose_opens_the_choice_for_a_builtin_agent() {
    assert!(!run(&["claude"]).choose);
    let options = run(&["--choose", "--headless", "codex"]);
    assert!(options.choose);
    assert_eq!(
        options.agent,
        AgentChoice::Builtin {
            agent: Agent::Codex,
            args: Vec::new(),
        }
    );
}

#[test]
fn choose_refuses_what_another_agent_could_not_take_over() {
    assert_eq!(
        parse(args(&["--choose", "--", "goose", "acp"])),
        Err(Error::ChooseCommand)
    );
    assert_eq!(
        parse(args(&["--choose", "claude", "--", "--model", "opus"])),
        Err(Error::ChooseCommand)
    );
    assert_eq!(
        parse(args(&["--choose", "--load", "s1", "claude"])),
        Err(Error::ChooseLoad)
    );
    assert_eq!(
        parse(args(&["--choose", "--resume", "s1", "claude"])),
        Err(Error::ChooseLoad)
    );
    assert_eq!(parse(args(&["--choose"])), Err(Error::MissingAgent));
}
