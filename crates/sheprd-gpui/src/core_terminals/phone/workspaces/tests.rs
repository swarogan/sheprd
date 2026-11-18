use super::*;

#[test]
fn a_checkout_with_changes_is_named_and_others_are_refused() {
    let dirty = crate::Error::Remote(sheprd_remote::Error::Work(work::Error::Dirty));
    assert_eq!(removal_failure(&dirty), Failure::Dirty);
    assert_eq!(
        removal_failure(&crate::Error::StaleWorkspace),
        Failure::Gone
    );
    assert_eq!(
        removal_failure(&crate::Error::EmptyWorkspaceLabel),
        Failure::Refused
    );
}
