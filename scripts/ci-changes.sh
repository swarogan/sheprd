#!/usr/bin/env bash
# Keep the required PR gate running while skipping native jobs for non-code edits.
# Documentation-only images are excluded by name so any new asset is treated
# as a build input until proven otherwise.
# The website workflow only publishes site/, which no build or test reads; the
# zizmor job still audits it on every run.
#
# Prints `code=` for the native jobs, `windows_package=` for the optimized
# Windows packaging and installer check, which is too slow to run for every
# change and covers only what the Windows release artifacts are made from,
# `windows_native=` for the Windows GUI tests, `macos_release=` for the macOS
# release builds, and `website=` for the Python-only sheprd.org and docs
# checks. A website-only change runs those and none of the native jobs.
#
# Every hosted job shares one account-wide runner limit, and macOS jobs a
# far smaller one, so pull requests skip the jobs their change cannot affect.
# Main always runs the Windows GUI tests: a release requires them there.
set -euo pipefail

if [[ "$EVENT" == workflow_dispatch ]] ||
   [[ ! "$BASE" =~ ^[0-9a-f]{40}$ ]] ||
   ! git cat-file -e "$BASE^{commit}" 2>/dev/null; then
  printf 'code=true\nwindows_package=true\nwindows_native=true\nmacos_release=true\nwebsite=true\n'
  exit 0
fi

# PRs use their merge base; pushes compare the complete pushed range.
if [[ "$EVENT" == pull_request ]]; then
  BASE="$(git merge-base "$BASE" "$HEAD_SHA")"
fi

# Prints true when any of the pathspecs changed in the range.
changed() {
  if git diff --quiet "$BASE" "$HEAD_SHA" -- "$@"; then
    printf 'false'
  else
    local status=$?
    # A Git error must fail detection, never silently skip required checks.
    [[ "$status" == 1 ]] || exit "$status"
    printf 'true'
  fi
}

code=$(changed \
  crates assets .cargo scripts .github/workflows \
  Cargo.toml Cargo.lock rust-toolchain.toml rustfmt.toml .rustfmt.toml \
  clippy.toml .clippy.toml justfile mise.toml cliff.toml \
  ':(exclude)**/*.md' \
  ':(exclude)assets/social-preview-*' \
  ':(exclude)assets/icons/sheprd-icon-badge.*' \
  ':(exclude).github/workflows/site.yml')
# Crates embed the skills they install and test the published relay list,
# so those reach the native jobs too, Markdown and all.
embedded=$(changed skills website/site/relays.json website/site/relays.json.asc)
[[ "$embedded" == true ]] && code=true
windows_package=$(changed \
  scripts/release/package-windows.py scripts/release/windows-installer.py \
  scripts/release/sheprd.iss scripts/release/installer site/install.ps1 \
  crates/sheprd-launcher crates/sheprd-gpui/src/lib.rs crates/sheprd-gpui/src/main.rs \
  crates/sheprd-gpui/build.rs crates/sheprd-gpui/windows_resource.rs \
  assets/icons/Sheprd.ico assets/icons/Sheprd-worktree.ico assets/icons/windows \
  .github/workflows/ci.yml)
# The Windows GUI tests launch the whole app, so they cover what sheprd.exe
# links, minus the crates it never links and each crate's own unit tests
# (`tests.rs`, `tests/`, `*_tests.rs`, `*_tests/`), which other jobs run.
# The second list adds back what the GUI job itself runs.
windows_native=$(changed \
  crates assets .cargo Cargo.toml Cargo.lock rust-toolchain.toml \
  ':(exclude)**/*.md' \
  ':(exclude)assets/social-preview-*' \
  ':(exclude)assets/icons/sheprd-icon-badge.*' \
  ':(exclude)crates/sheprd-acp' ':(exclude)crates/sheprd-apns' \
  ':(exclude)crates/sheprd-automate' ':(exclude)crates/sheprd-launcher' \
  ':(exclude)crates/sheprd-nearby' ':(exclude)crates/sheprd-push-relay' \
  ':(exclude)crates/sheprd-relay-agent' ':(exclude)crates/sheprd-tunnel' \
  ':(exclude)crates/test-support' \
  ':(exclude,glob)crates/**/tests/**' ':(exclude,glob)crates/**/tests.rs' \
  ':(exclude,glob)crates/**/*_tests.rs' ':(exclude,glob)crates/**/*_tests/**')
if [[ "$windows_native" == false ]]; then
  windows_native=$(changed \
    crates/sheprd-gpui/tests/live_gui.rs crates/sheprd-gpui/tests/support \
    crates/test-support/sandbox.rs crates/sheprd-updater/src/windows/tests.rs \
    scripts/install-herdr-windows.py scripts/set-display-resolution.ps1 \
    scripts/ci-changes.sh .github/workflows/ci.yml)
fi
[[ "$EVENT" == pull_request ]] || windows_native=$code
# The macOS release builds differ from the debug checks in their profile,
# target (x86_64 under Rosetta), and release-mode CLI suite, so they run when
# what those depend on changes; the release workflow builds them regardless.
# `crates/*/*.rs` is each build script and the helpers beside it, which
# `build.rs` loads with `mod` (`build_identity.rs`, `locales.rs`, ...).
macos_release=$(changed \
  ':(glob)**/Cargo.toml' Cargo.lock rust-toolchain.toml .cargo \
  ':(glob)crates/*/*.rs' crates/sheprd-gpui/tests/cli.rs crates/sheprd-gpui/tests/cli \
  scripts/ci-changes.sh .github/workflows/ci.yml)
# The docs link into READMEs, docs/ and the example config, so any Markdown or
# build input can break a page; the check takes seconds.
website=$(changed website site docs ':(glob)**/*.md' .github/workflows/ci.yml)
[[ "$code" == true ]] && website=true
printf 'code=%s\nwindows_package=%s\nwindows_native=%s\nmacos_release=%s\nwebsite=%s\n' \
  "$code" "$windows_package" "$windows_native" "$macos_release" "$website"
