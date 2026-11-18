use super::*;

#[test]
fn an_installed_core_precedes_the_bundled_fallback() {
    let bundle = tempfile::tempdir().unwrap();
    let external = tempfile::tempdir().unwrap();
    let exe = bundle.path().join("sheprd");
    let path = std::env::join_paths([external.path()]).unwrap();
    for program in ["herdr-core", "herdr-core.exe"] {
        let bundled = bundle.path().join(program);
        let installed = external.path().join(program);
        helper(&bundled);
        helper(&installed);
        assert_eq!(
            locate_in(program, Some(&exe), Some(&path)),
            Some(installed.clone())
        );
        std::fs::remove_file(installed).unwrap();
        assert_eq!(locate_in(program, Some(&exe), Some(&path)), Some(bundled));
    }
}
