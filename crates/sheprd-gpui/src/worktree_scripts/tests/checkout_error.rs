use super::*;

#[test]
fn a_failed_kept_checkout_lookup_uses_the_translatable_git_operation() {
    let temp = tempfile::tempdir().unwrap();
    let missing = temp.path().join("missing");
    let error =
        super::super::kept_script_checkout(missing.to_str().unwrap(), &AtomicBool::new(false))
            .unwrap_err();
    let crate::Error::Vcs(sheprd_vcs::Error::Git(crate::git::Error::Failed { operation, .. })) =
        &error
    else {
        panic!("expected a Git checkout lookup failure, got {error:?}");
    };
    assert_eq!(*operation, "find the checkout");
    crate::i18n::errors::tests::assert_localized(&[error]);
}
