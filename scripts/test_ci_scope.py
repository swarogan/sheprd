#!/usr/bin/env python3
"""Tests for scripts/ci-scope.py, which picks a CI run's platforms and crates."""

import importlib.util
import os
import tempfile
import unittest
from pathlib import Path

SCRIPT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "ci-scope.py")
SPEC = importlib.util.spec_from_file_location("ci_scope", SCRIPT)
SCOPE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(SCOPE)

# protocol <- client <- gpui; client's tests also use test-kit; relay stands alone.
MANIFESTS = {
    "Cargo.toml": '[workspace]\nmembers = ["crates/*"]\n'
                  '[workspace.dependencies]\nprotocol = { path = "crates/protocol" }\n',
    "crates/protocol/Cargo.toml": '[package]\nname = "protocol"\n',
    "crates/test-kit/Cargo.toml": '[package]\nname = "test-kit"\n',
    "crates/client/Cargo.toml": '[package]\nname = "client"\n'
                                '[dependencies]\nprotocol = { workspace = true }\n'
                                '[target.\'cfg(unix)\'.dev-dependencies]\nkit = { package = "test-kit", path = "../test-kit" }\n',
    "crates/sheprd-gpui/Cargo.toml": '[package]\nname = "sheprd-gpui"\n'
                                     '[build-dependencies]\nclient = { path = "../client" }\n'
                                     '[dependencies]\nserde = "1"\n',
    "crates/relay/Cargo.toml": '[package]\nname = "relay"\n',
}


class CiScopeTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for path, text in MANIFESTS.items():
                (root / path).parent.mkdir(parents=True, exist_ok=True)
                (root / path).write_text(text)
            cls.members, cls.dependents = SCOPE.workspace(root)

    def plan(self, changed, event="pull_request", labels=(), lines=()):
        return SCOPE.plan(event, labels, changed, lines, self.members, self.dependents)

    def assert_full(self, outputs):
        self.assertEqual(outputs, {"full": "true", "cargo_scope": "--workspace", "gpui": "true"})

    def test_reads_every_dependency_kind_and_rename(self):
        self.assertEqual(self.members["crates/relay"], "relay")
        self.assertEqual(self.dependents["protocol"], {"client"})
        self.assertEqual(self.dependents["test-kit"], {"client"})
        self.assertEqual(self.dependents["client"], {"sheprd-gpui"})
        self.assertNotIn("serde", self.dependents)

    def test_crate_change_reaches_its_dependents_on_linux_only(self):
        outputs = self.plan(["crates/protocol/src/lib.rs"])
        self.assertEqual(outputs["full"], "false")
        self.assertEqual(outputs["cargo_scope"], "-p client -p protocol -p sheprd-gpui")
        self.assertEqual(outputs["gpui"], "true")

    def test_leaf_crate_change_skips_the_app(self):
        outputs = self.plan(["crates/relay/src/main.rs", "crates/relay/README.md"])
        self.assertEqual(outputs["cargo_scope"], "-p relay")
        self.assertEqual(outputs["gpui"], "false")

    def test_shared_inputs_reach_the_whole_workspace(self):
        for path in ("assets/icons/app.png", "scripts/check-sorted.py",
                     "crates/test-support/sandbox.rs", "crates/embed-resource/src/lib.rs",
                     "skills/herdr-mods/SKILL.md", "website/site/relays.json",
                     "website/site/relays.json.asc"):
            with self.subTest(path=path):
                outputs = self.plan(["crates/relay/src/main.rs", path])
                self.assertEqual(outputs["full"], "false")
                self.assertEqual(outputs["cargo_scope"], "--workspace")
                self.assertEqual(outputs["gpui"], "true")

    def test_unread_paths_select_no_crate(self):
        outputs = self.plan(["docs/cli.md", "README.md", "site/index.html",
                             "website/site/index.html", "website/docs/install.md",
                             ".github/workflows/ios.yml"])
        self.assertEqual(outputs["cargo_scope"], "")
        self.assertEqual(outputs["gpui"], "false")

    def test_main_merge_queue_and_dispatch_run_everything(self):
        for event in ("push", "merge_group", "workflow_dispatch"):
            with self.subTest(event=event):
                self.assert_full(self.plan(["crates/relay/src/main.rs"], event=event))

    def test_missing_base_runs_everything(self):
        self.assert_full(self.plan(None))

    def test_platform_labels_run_everything(self):
        for label in ("windows", "macos", "full-ci"):
            with self.subTest(label=label):
                self.assert_full(self.plan(["crates/relay/src/main.rs"], labels=["bug", label]))
        self.assertEqual(self.plan(["crates/relay/src/main.rs"], labels=["linux"])["full"], "false")

    def test_dependencies_and_toolchain_run_everything(self):
        # A target table, or a crate only one platform compiles, changes what
        # that platform builds; the lockfile carries both.
        for path in ("Cargo.lock", "Cargo.toml", "crates/client/Cargo.toml",
                     "rust-toolchain.toml", ".cargo/config.toml"):
            with self.subTest(path=path):
                self.assert_full(self.plan([path]))

    def test_platform_paths_run_everything(self):
        for path in ("crates/sheprd-gpui/src/avatars/cache/windows.rs", "macos/sheprd-nearby/main.swift",
                     "crates/sheprd-gpui/windows.manifest", "crates/client/src/macos_keychain.rs",
                     ".github/workflows/ci.yml", "scripts/ci-scope.py", "scripts/ci-changes.sh",
                     "scripts/set-display-resolution.ps1"):
            with self.subTest(path=path):
                self.assert_full(self.plan([path]))
        for path in ("crates/client/src/window.rs", "crates/client/src/windowing.rs"):
            with self.subTest(path=path):
                self.assertEqual(self.plan([path])["full"], "false")

    def test_platform_cfg_lines_run_everything(self):
        for line in ('+#[cfg(windows)]', '-#[cfg(target_os = "macos")]', '+    if cfg!(unix) {',
                     '+#[cfg(all(test, not(windows)))]', '+#[cfg_attr(windows, allow(dead_code))]',
                     '-    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {'):
            with self.subTest(line=line):
                self.assert_full(self.plan(["crates/client/src/lib.rs"], lines=[line]))
        self.assertEqual(self.plan(["crates/client/src/lib.rs"],
                                   lines=['+#[cfg(test)]', '+// a window opens'])["full"], "false")


if __name__ == "__main__":
    unittest.main()
