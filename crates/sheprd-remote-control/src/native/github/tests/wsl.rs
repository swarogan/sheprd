//! WSL workspaces' folders are not this computer's to read with its own
//! Git, so checkout discovery and `home` leave them out.

#[test]
fn only_workspaces_inside_a_wsl_distribution_are_left_out() {
    assert!(crate::hub::in_wsl("herdr:default@Ubuntu:w1"));
    assert!(!crate::hub::in_wsl("w1"));
    assert!(!crate::hub::in_wsl("herdr:work:w1"));
}
