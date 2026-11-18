use super::*;
use control::ControlCommand;

fn command(args: &[&str]) -> Result<ControlCommand, CliError> {
    match LaunchOptions::parse(args.iter().copied()) {
        Ok(LaunchOptions {
            mode: LaunchMode::Control(invocation),
            ..
        }) => Ok(invocation.command),
        Ok(other) => panic!("{args:?} parsed as {:?}", other.mode),
        Err(error) => Err(error),
    }
}

#[test]
fn usage_lists_and_pick_picks() {
    assert_eq!(command(&["usage"]).unwrap(), ControlCommand::Usage);
    assert_eq!(
        command(&["usage", "pick", "--json"]).unwrap(),
        ControlCommand::UsagePick
    );
    assert!(command(&["usage", "nope"]).is_err());
}
