use super::*;
use anyhow::Context as _;
use std::os::unix::fs::symlink;

fn applications() -> anyhow::Result<(tempfile::TempDir, PathBuf)> {
    let root = tempfile::tempdir()?;
    let legacy = root.path().join("Herdr.app");
    fs::create_dir_all(legacy.join("Contents/MacOS"))?;
    fs::write(legacy.join("Contents/version"), b"current")?;
    Ok((root, legacy))
}

fn standalone(bundle: &Path) -> Option<LegacyBundle> {
    match Legacy::at(bundle, |_| None)? {
        Legacy::Standalone(legacy) => Some(legacy),
        Legacy::Homebrew(_) => None,
    }
}

#[test]
fn standalone_legacy_bundle_moves_to_the_current_name() -> anyhow::Result<()> {
    let (root, legacy) = applications()?;
    let found = standalone(&legacy).context("not offered")?;
    assert_eq!(found.bundle, legacy);
    assert_eq!(found.renamed(), root.path().join("Sheprd.app"));
    Ok(())
}

#[test]
fn bundle_with_any_other_name_is_left_alone() -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    for name in ["Sheprd.app", "Herdr 2.app", "herdr.app"] {
        let bundle = root.path().join(name);
        fs::create_dir_all(&bundle)?;
        assert_eq!(Legacy::at(&bundle, |_| Some("herdr-gpui")), None, "{name}");
    }
    Ok(())
}

#[test]
fn homebrew_bundle_is_told_to_reinstall_from_the_current_cask() -> anyhow::Result<()> {
    let (_root, legacy) = applications()?;
    let mut asked = None;
    let found = Legacy::at(&legacy, |bundle| {
        asked = Some(bundle.to_owned());
        Some("herdr-gpui")
    });
    assert_eq!(asked.as_deref(), Some(legacy.as_path()));
    let Some(Legacy::Homebrew(homebrew)) = found else {
        anyhow::bail!("not a Homebrew install: {found:?}");
    };
    assert_eq!(homebrew.bundle, legacy);
    assert_eq!(
        homebrew.commands(),
        "brew uninstall --cask herdr-gpui\nbrew install sheprd-labs/tap/sheprd"
    );
    Ok(())
}

#[test]
fn homebrew_bundle_is_told_even_beside_an_existing_sheprd_app() -> anyhow::Result<()> {
    let (root, legacy) = applications()?;
    fs::create_dir(root.path().join("Sheprd.app"))?;
    let found = Legacy::at(&legacy, |_| Some("herdr-gpui"));
    assert!(matches!(found, Some(Legacy::Homebrew(_))), "{found:?}");
    Ok(())
}

#[test]
fn existing_sheprd_app_is_never_replaced() -> anyhow::Result<()> {
    let (root, legacy) = applications()?;
    let current = root.path().join("Sheprd.app");
    fs::create_dir(&current)?;
    assert_eq!(Legacy::at(&legacy, |_| None), None);
    // Nor a link standing at that name.
    fs::remove_dir(&current)?;
    symlink("elsewhere", &current)?;
    assert_eq!(Legacy::at(&legacy, |_| None), None);
    Ok(())
}

#[test]
fn already_linked_old_path_is_not_offered_again() -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    let legacy = root.path().join("Herdr.app");
    fs::create_dir(root.path().join("Moved.app"))?;
    symlink("Moved.app", &legacy)?;
    assert_eq!(Legacy::at(&legacy, |_| None), None);
    Ok(())
}

#[test]
fn rename_moves_the_bundle_and_links_its_old_path() -> anyhow::Result<()> {
    let (root, legacy) = applications()?;
    let found = standalone(&legacy).context("not offered")?;
    found.rename()?;
    let current = root.path().join("Sheprd.app");
    assert_eq!(fs::read(current.join("Contents/version"))?, b"current");
    assert_eq!(fs::read_link(&legacy)?, Path::new("Sheprd.app"));
    assert!(legacy.join("Contents/MacOS").is_dir());
    // Once moved, the app is no longer offered the move.
    assert_eq!(Legacy::at(&legacy, |_| None), None);
    assert_eq!(Legacy::at(&current, |_| None), None);
    Ok(())
}

#[test]
fn rename_refuses_a_sheprd_app_that_appeared_since_asking() -> anyhow::Result<()> {
    let (root, legacy) = applications()?;
    let found = standalone(&legacy).context("not offered")?;
    let current = root.path().join("Sheprd.app");
    fs::create_dir(&current)?;
    let error = found.rename().err().context("renamed over Sheprd.app")?;
    assert!(
        matches!(&error, Error::Io(source) if source.kind() == ErrorKind::AlreadyExists),
        "{error:?}"
    );
    assert!(legacy.join("Contents/version").is_file());
    assert_eq!(fs::read_dir(&current)?.count(), 0);
    Ok(())
}
