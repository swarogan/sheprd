"""What keeps pull requests from waiting on the account-wide runner limits."""

from pathlib import Path
import re
import unittest

from test_ci_changes import GitFixture
from test_ci_policy import JOBS, PR_JOBS, condition_matches


ROOT = Path(__file__).resolve().parents[3]
WORKFLOWS = ROOT / ".github/workflows"
DRAFT = "github.event.pull_request.draft != true"


def jobs(text):
    sections = re.split(r"^  ([a-z-]+):\n", text.split("\njobs:\n", 1)[1], flags=re.M)
    return dict(zip(sections[1::2], sections[2::2]))


def needs(job):
    match = re.search(r"^    needs: (.+)$", job, re.M)
    return set(match[1].strip("[]").split(", ")) if match else set()


class Drafts(unittest.TestCase):
    def test_every_pull_request_workflow_skips_drafts_until_ready(self):
        for path in sorted(WORKFLOWS.glob("*.yml")):
            text = path.read_text()
            if not re.search(r"^  pull_request:$", text, re.M):
                continue
            with self.subTest(workflow=path.name):
                trigger = re.search(r"^  pull_request:\n((?:    .*\n)+)", text, re.M)[1]
                self.assertRegex(trigger, r"types: \[opened, synchronize, reopened, "
                                          r"(?:labeled, )?ready_for_review\]")
                for name, job in jobs(text).items():
                    # A job that waits on another is skipped with it.
                    if not needs(job):
                        condition = re.search(r"^    if: (.+(?:\n      .+)*)", job, re.M)
                        self.assertIsNotNone(condition, name)
                        # Jobs limited to other events never see a draft.
                        if ("github.event_name ==" in condition[1]
                                and "'pull_request'" not in condition[1]):
                            continue
                        self.assertIn(DRAFT, condition[1], name)

    def test_ci_runs_nothing_for_a_draft(self):
        context = {
            "github.repository": "sheprd-labs/sheprd", "github.event_name": "pull_request",
            "github.ref": "refs/pull/456/merge", "github.actor": "penso",
            "github.triggering_actor": "penso",
        }
        for draft in (True, False):
            for job in ("zizmor", "commits", "checks-passed"):
                with self.subTest(job=job, draft=draft):
                    self.assertEqual(condition_matches(
                        job, {**context, "github.event.pull_request.draft": draft}), not draft)
        for name, job in JOBS.items():
            if name not in ("zizmor", "commits", "checks-passed"):
                self.assertIn("commits", needs(job), name)


class Queue(unittest.TestCase):
    def test_only_the_gate_waits_for_the_workflow_audit(self):
        for name, job in JOBS.items():
            if name != "checks-passed":
                self.assertNotIn("zizmor", needs(job), name)
        self.assertEqual(needs(JOBS["checks-passed"]),
                         {name for name in JOBS if name not in ("checks-passed", "build")})
        self.assertTrue(set(PR_JOBS) <= set(JOBS))

    def test_pull_requests_run_one_macos_leg_and_main_both(self):
        # A pull request with every platform runs one macOS leg; one without
        # runs Linux alone (scripts/ci-scope.py); the merge queue and main run
        # both macOS legs.
        exclude = re.search(r"^        exclude: (.+)$", JOBS["checks"], re.M)[1]
        self.assertEqual(exclude, "${{ fromJSON(needs.commits.outputs.full != 'true' && "
                         "'[{\"runner\":\"macos-15\"},{\"runner\":\"ubuntu-24.04-arm\"}]' || "
                         "github.event_name == 'pull_request' && "
                         "'[{\"runner\":\"macos-15\",\"features\":\"all\"}]' || '[]') }}")


class WindowsNativeChanges(GitFixture):
    """The Windows GUI tests run for what the app they launch is built from."""

    def test_app_inputs_run_the_gui_tests(self):
        for path in ("crates/sheprd-gpui/src/app.rs", "crates/sheprd-client/src/lib.rs",
                     "crates/sheprd-updater/src/windows/tests.rs", "assets/app.css",
                     "Cargo.lock", "crates/sheprd-gpui/Cargo.toml",
                     "crates/sheprd-gpui/tests/live_gui.rs",
                     "crates/sheprd-gpui/tests/support/clipboard_guard.rs",
                     "crates/test-support/sandbox.rs", "scripts/install-herdr-windows.py",
                     "scripts/ci-changes.sh", ".github/workflows/ci.yml"):
            self.base = self.git("rev-parse", "HEAD").strip()
            self.commit_path(path)
            self.assertEqual(self.outputs()["windows_native"], "true", path)

    def test_unit_tests_and_other_programs_skip_the_gui_tests(self):
        for path in ("crates/sheprd-gpui/src/menu/tests.rs",
                     "crates/sheprd-gpui/src/menu/tests/close.rs",
                     "crates/sheprd-gpui/src/endpoint/lifecycle_tests.rs",
                     "crates/sheprd-gpui/src/menu/workspace_tests/kept.rs",
                     "crates/sheprd-gpui/tests/cli.rs", "crates/sheprd-acp/src/main.rs",
                     "crates/sheprd-relay-agent/src/main.rs", "crates/sheprd-tunnel/src/lib.rs",
                     "crates/test-support/pe.rs", "scripts/release/build.sh",
                     "crates/sheprd-gpui/README.md"):
            self.base = self.git("rev-parse", "HEAD").strip()
            self.commit_path(path)
            self.assertEqual(self.outputs()["windows_native"], "false", path)

    def test_main_runs_the_gui_tests_for_any_code_change(self):
        self.commit_path("crates/sheprd-gpui/src/menu/tests.rs")
        self.assertEqual(self.outputs("push")["windows_native"], "true")
        self.base = self.git("rev-parse", "HEAD").strip()
        self.commit_path("README.md")
        self.assertEqual(self.outputs("push")["windows_native"], "false")

    def test_manual_and_unknown_base_run_the_gui_tests(self):
        self.assertEqual(self.outputs("workflow_dispatch")["windows_native"], "true")
        self.assertEqual(self.outputs("pull_request", "0" * 40)["windows_native"], "true")


class MacosReleaseChanges(GitFixture):
    """The macOS release builds run for what makes them differ from the checks."""

    def test_release_inputs_run_the_macos_builds(self):
        for path in ("Cargo.lock", "Cargo.toml", "crates/sheprd-gpui/Cargo.toml",
                     "rust-toolchain.toml", ".cargo/config.toml", "crates/sheprd-gpui/build.rs",
                     "crates/sheprd-gpui/build_identity.rs", "crates/sheprd-gpui/locales.rs",
                     "crates/sheprd-gpui/release_version.rs", "crates/sheprd-launcher/build.rs",
                     "crates/sheprd-gpui/tests/cli.rs", "crates/sheprd-gpui/tests/cli/help.rs",
                     ".github/workflows/ci.yml"):
            self.base = self.git("rev-parse", "HEAD").strip()
            self.commit_path(path)
            self.assertEqual(self.outputs()["macos_release"], "true", path)

    def test_source_changes_skip_the_macos_builds(self):
        for path in ("crates/sheprd-gpui/src/app.rs", "crates/sheprd-gpui/src/main.rs",
                     "crates/sheprd-gpui/tests/live_gui.rs", "assets/app.css", "README.md"):
            self.base = self.git("rev-parse", "HEAD").strip()
            self.commit_path(path)
            self.assertEqual(self.outputs()["macos_release"], "false", path)

    def test_manual_and_unknown_base_run_the_macos_builds(self):
        self.assertEqual(self.outputs("workflow_dispatch")["macos_release"], "true")
        self.assertEqual(self.outputs("pull_request", "0" * 40)["macos_release"], "true")


if __name__ == "__main__":
    unittest.main()
