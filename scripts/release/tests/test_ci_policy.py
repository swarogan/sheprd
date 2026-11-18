"""CI eligibility and credential boundaries; GitHub enforces fork approval online."""

import ast
import itertools
import os
from pathlib import Path
import re
import subprocess
import unittest


ROOT = Path(__file__).resolve().parents[3]
WORKFLOW = (ROOT / ".github/workflows/ci.yml").read_text()
SECTIONS = re.split(r"^  ([a-z-]+):\n", WORKFLOW.split("\njobs:\n", 1)[1], flags=re.M)
JOBS = dict(zip(SECTIONS[1::2], SECTIONS[2::2]))
PR_JOBS = ("zizmor", "commits", "checks", "windows", "windows-native", "build-macos", "website",
           "checks-passed")
# Jobs that never run on main: the release workflow builds those targets.
PR_ONLY_JOBS = ("build-macos",)


def condition_matches(job, context, jobs=JOBS):
    # Interpret only the boolean/equality subset used by these job conditions.
    # Unsupported syntax fails rather than executing workflow text as Python.
    expression = re.search(r"^    if: >-\n((?:      .+\n)+)", jobs[job], re.M)[1]
    expression = " ".join(expression.split()).replace("&&", "and").replace("||", "or")

    def evaluate(node):
        if isinstance(node, ast.Constant) and isinstance(node.value, str):
            return node.value
        if isinstance(node, ast.Name) and node.id in ("true", "false"):
            return node.id == "true"
        if isinstance(node, ast.Attribute):
            return context[ast.unparse(node)]
        if isinstance(node, ast.BoolOp):
            values = [evaluate(value) for value in node.values]
            if isinstance(node.op, ast.And):
                return all(values)
            if isinstance(node.op, ast.Or):
                return any(values)
        if isinstance(node, ast.Compare) and len(node.ops) == 1:
            left, right = evaluate(node.left), evaluate(node.comparators[0])
            if isinstance(node.ops[0], ast.Eq):
                return left == right
            if isinstance(node.ops[0], ast.NotEq):
                return left != right
        if (isinstance(node, ast.Call) and isinstance(node.func, ast.Name)
                and node.func.id == "always" and not node.args and not node.keywords):
            return True
        raise AssertionError(f"Unsupported CI expression: {ast.dump(node)}")

    return evaluate(ast.parse(expression, mode="eval").body)


class CiPolicy(unittest.TestCase):
    def context(self, **overrides):
        return {
            "github.repository": "sheprd-labs/sheprd",
            "github.event_name": "pull_request",
            "github.ref": "refs/pull/456/merge",
            "github.actor": "larry0x",
            "github.triggering_actor": "larry0x",
            "github.event.pull_request.user.login": "larry0x",
            "github.event.pull_request.head.repo.full_name": "larry0x/herdr-gpui",
            "github.event.pull_request.draft": False,
            "needs.commits.outputs.code": "true",
            "needs.commits.outputs.full": "true",
            "needs.commits.outputs.macos_release": "true",
            "needs.commits.outputs.website": "true",
            "needs.commits.outputs.windows_native": "true",
            "needs.commits.outputs.windows_package": "true",
            **overrides,
        }

    def test_fork_pr_can_run_after_github_approval_or_owner_rerun(self):
        # Approval does not rewrite actor/author/source repo. An owner rerun
        # changes triggering_actor only; neither case may skip the PR checks.
        for actor in ("larry0x", "penso"):
            context = self.context(**{"github.triggering_actor": actor})
            for job in PR_JOBS:
                with self.subTest(actor=actor, job=job):
                    self.assertTrue(condition_matches(job, context))
            self.assertFalse(condition_matches("build", context))

    def test_owner_pr_still_runs_checks_but_not_release_build(self):
        context = self.context(**{
            "github.actor": "penso", "github.triggering_actor": "penso",
            "github.event.pull_request.user.login": "penso",
            "github.event.pull_request.head.repo.full_name": "sheprd-labs/sheprd",
        })
        for job in PR_JOBS:
            self.assertTrue(condition_matches(job, context), job)
        self.assertFalse(condition_matches("build", context))

    def test_non_pr_events_require_owner_main_and_expected_event(self):
        for event, ref, actor, triggering_actor in itertools.product(
                ("push", "workflow_dispatch", "pull_request_target", "workflow_run"),
                ("refs/heads/main", "refs/heads/topic", "refs/tags/v1"),
                ("penso", "larry0x"), ("penso", "larry0x")):
            context = self.context(**{
                "github.event_name": event, "github.ref": ref,
                "github.actor": actor, "github.triggering_actor": triggering_actor,
            })
            allowed = (event in ("push", "workflow_dispatch") and ref == "refs/heads/main"
                       and actor == "penso" and triggering_actor == "penso")
            for job in JOBS:
                with self.subTest(job=job, context=context):
                    self.assertEqual(condition_matches(job, context),
                                     allowed and job not in PR_ONLY_JOBS)

    def test_fast_pull_request_skips_only_other_platforms(self):
        # Without every platform, a pull request still runs the Linux checks.
        for full in ("false", ""):
            context = self.context(**{"needs.commits.outputs.full": full})
            for job in PR_JOBS:
                self.assertEqual(condition_matches(job, context),
                                 job not in ("windows", "windows-native", "build-macos"), (job, full))

    def test_merge_queue_runs_every_platform_but_no_main_release_build(self):
        # The queue's ref and actor are GitHub's; it checks what reaches main.
        for actor in ("larry0x", "penso", "github-merge-queue[bot]"):
            context = self.context(**{
                "github.event_name": "merge_group",
                "github.ref": "refs/heads/gh-readonly-queue/main/pr-456-0123456789abcdef",
                "github.actor": actor, "github.triggering_actor": actor,
            })
            for job in JOBS:
                with self.subTest(actor=actor, job=job):
                    self.assertEqual(condition_matches(job, context), job != "build")

    def test_other_repositories_cannot_run_jobs(self):
        for event in ("pull_request", "merge_group", "push", "workflow_dispatch"):
            context = self.context(**{
                "github.repository": "larry0x/herdr-gpui", "github.event_name": event,
                "github.ref": "refs/heads/main", "github.actor": "penso",
                "github.triggering_actor": "penso",
            })
            for job in JOBS:
                self.assertFalse(condition_matches(job, context), (job, event))

    def test_pretransfer_repository_still_runs_pull_request_checks(self):
        context = self.context(**{"github.repository": "penso/herdr-gpui"})
        for job in PR_JOBS:
            self.assertTrue(condition_matches(job, context), job)

    def test_owner_only_workflows_allow_both_repository_locations_but_not_forks(self):
        for workflow in ("supply-chain.yml",):
            text = (ROOT / ".github/workflows" / workflow).read_text()
            sections = re.split(r"^  ([a-z-]+):\n", text.split("\njobs:\n", 1)[1], flags=re.M)
            jobs = dict(zip(sections[1::2], sections[2::2]))
            for repository, head in itertools.product(
                    ("penso/herdr-gpui", "sheprd-labs/sheprd", "other/sheprd"),
                    ("penso/herdr-gpui", "sheprd-labs/sheprd", "other/sheprd")):
                context = self.context(**{
                    "github.repository": repository, "github.actor": "penso",
                    "github.triggering_actor": "penso",
                    "github.event.pull_request.user.login": "penso",
                    "github.event.pull_request.head.repo.full_name": head,
                })
                expected = repository != "other/sheprd" and head == repository
                for job in jobs:
                    with self.subTest(workflow=workflow, job=job, repository=repository, head=head):
                        self.assertEqual(condition_matches(job, context, jobs), expected)

    def test_path_selection_only_skips_platform_jobs(self):
        for code in ("false", ""):
            context = self.context(**{"needs.commits.outputs.code": code})
            for job in PR_JOBS:
                self.assertEqual(condition_matches(job, context),
                                 job not in ("checks", "windows"), (job, code))
        # The GUI tests and the macOS release builds follow their own inputs.
        for output, selected in (("windows_native", "windows-native"),
                                 ("macos_release", "build-macos")):
            for value in ("false", ""):
                context = self.context(**{f"needs.commits.outputs.{output}": value})
                for job in PR_JOBS:
                    self.assertEqual(condition_matches(job, context), job != selected,
                                     (job, output, value))
        for website in ("false", ""):
            context = self.context(**{"needs.commits.outputs.website": website})
            for job in PR_JOBS:
                self.assertEqual(condition_matches(job, context), job != "website", (job, website))

    def test_required_gate_rejects_failures_skips_and_missing_detection(self):
        # Run the actual shell gate, including the docs-only exception.
        script = JOBS["checks-passed"].split("        run: |\n", 1)[1]
        script = "\n".join(line[10:] for line in script.splitlines() if line.startswith("          "))

        def gate(code, checks, windows, native, commits, package_inputs="false", package="skipped",
                 event="push", macos=None, site_inputs="false", site="skipped", full="true",
                 native_inputs=None, macos_inputs=None, zizmor="success"):
            if native_inputs is None:
                native_inputs = code if code in ("true", "false") else "false"
            if macos_inputs is None:
                macos_inputs = native_inputs
            if macos is None:
                before_merge = event in ("pull_request", "merge_group")
                selected = code == macos_inputs == full == "true" and before_merge
                macos = "success" if selected else "skipped"
            return subprocess.run(
                ["bash", "-e", "-c", script], capture_output=True,
                env={**os.environ, "CODE": code, "RESULT": checks, "WINDOWS": windows,
                     "NATIVE": native, "NATIVE_INPUTS": native_inputs, "COMMITS": commits,
                     "FULL": full,
                     "PACKAGE_INPUTS": package_inputs, "PACKAGE": package,
                     "EVENT": event, "MACOS_RELEASE": macos, "MACOS_INPUTS": macos_inputs,
                     "SITE_INPUTS": site_inputs, "SITE": site, "ZIZMOR": zizmor},
            ).returncode == 0

        for code, checks, windows, commits in itertools.product(
                ("true", "false", ""), ("success", "failure", "cancelled", "skipped"),
                ("success", "failure", "cancelled", "skipped"), ("success", "failure", "skipped")):
            expected = commits == "success" and (
                (code == "true" and checks == windows == "success")
                or (code == "false" and checks == windows == "skipped"))
            # With every input changed, the native Windows job follows the
            # same rule as the Windows check.
            self.assertEqual(gate(code, checks, windows, windows, commits), expected,
                             (code, checks, windows, commits))
        for native in ("failure", "cancelled", "skipped"):
            self.assertFalse(gate("true", "success", "success", native, "success"), native)
        self.assertFalse(gate("false", "skipped", "skipped", "success", "success"))
        # The workflow audit runs beside the other jobs, so only the gate holds it.
        for zizmor in ("failure", "cancelled", "skipped"):
            self.assertFalse(gate("true", "success", "success", "success", "success",
                                  zizmor=zizmor), zizmor)
        # The GUI tests run when their inputs change, and must then succeed;
        # otherwise they must have been skipped, and missing detection fails.
        for inputs, native in itertools.product(
                ("true", "false", ""), ("success", "failure", "cancelled", "skipped")):
            expected = ((inputs == "true" and native == "success")
                        or (inputs == "false" and native == "skipped"))
            self.assertEqual(gate("true", "success", "success", native, "success",
                                  native_inputs=inputs, macos_inputs="false"),
                             expected, (inputs, native))
        # The Windows package job runs only when its inputs change, and must
        # then succeed; missing detection fails like the platform jobs'.
        for inputs, package in itertools.product(
                ("true", "false", ""), ("success", "failure", "cancelled", "skipped")):
            expected = ((inputs == "true" and package == "success")
                        or (inputs == "false" and package == "skipped"))
            self.assertEqual(gate("true", "success", "success", "success", "success", inputs, package),
                             expected, (inputs, package))
        # A pull request may run Linux alone: the other platforms must then
        # have been skipped. Main and the merge queue must run every one, and
        # selection that failed to say fails the gate.
        for event, full, windows in itertools.product(
                ("pull_request", "merge_group", "push", "workflow_dispatch"), ("true", "false", ""),
                ("success", "failure", "skipped")):
            expected = (full == "true" and windows == "success") or (
                event == "pull_request" and full == "false" and windows == "skipped")
            self.assertEqual(gate("true", "success", windows, windows, "success", event=event, full=full),
                             expected, (event, full, windows))
        for event, full in itertools.product(("merge_group", "push"), ("false", "")):
            self.assertFalse(gate("false", "skipped", "skipped", "skipped", "success",
                                  event=event, full=full), (event, full))
        # The macOS release builds run, and must pass, before merge with every
        # platform when their inputs change only; anywhere else they must
        # have been skipped.
        for code, inputs, event, macos in itertools.product(
                ("true", "false"), ("true", "false"),
                ("pull_request", "merge_group", "push", "workflow_dispatch"),
                ("success", "failure", "cancelled", "skipped")):
            if code == "false" and inputs == "true":
                continue  # Its inputs are code, so detection never reports this.
            platform = "success" if code == "true" else "skipped"
            before_merge = event in ("pull_request", "merge_group")
            selected = code == inputs == "true" and before_merge
            expected = macos == ("success" if selected else "skipped")
            self.assertEqual(gate(code, platform, platform, platform, "success", event=event,
                                  macos=macos, native_inputs=code, macos_inputs=inputs),
                             expected, (code, inputs, event, macos))
        # The website job runs when the site or docs change, and must then
        # succeed; missing detection fails like the platform jobs'.
        for inputs, site in itertools.product(
                ("true", "false", ""), ("success", "failure", "cancelled", "skipped")):
            expected = ((inputs == "true" and site == "success")
                        or (inputs == "false" and site == "skipped"))
            self.assertEqual(gate("true", "success", "success", "success", "success",
                                  site_inputs=inputs, site=site),
                             expected, (inputs, site))

    def test_ci_has_no_privileged_trigger_or_credentials(self):
        triggers = WORKFLOW.split("\npermissions:", 1)[0]
        self.assertEqual(re.findall(r"^  (\w+):", triggers, re.M),
                         ["push", "pull_request", "merge_group", "workflow_dispatch"])
        self.assertIn("\npermissions: {}\n", WORKFLOW)
        for name, job in JOBS.items():
            with self.subTest(job=name):
                permissions = re.search(r"^    permissions:([^\n]*\n(?:      [^\n]*\n)*)", job, re.M)[1]
                self.assertIn(permissions.strip(), ("{}", "contents: read"))
                for forbidden in ("secrets.", "secrets[", "secrets:", "environment:", "id-token:"):
                    self.assertNotIn(forbidden, job)
                if "actions/checkout@" in job:
                    self.assertIn("persist-credentials: false", job)
                if "Swatinem/rust-cache@" in job:
                    self.assertIn("save-if: ${{ github.ref == 'refs/heads/main' }}", job)
        # Keep runners a fixed hosted set; never accept a PR-controlled label.
        self.assertEqual(re.findall(r"^    runs-on: (.+)$", WORKFLOW, re.M), [
            "ubuntu-latest", "ubuntu-24.04", "${{ matrix.runner }}",
            "${{ matrix.runner }}", "${{ matrix.runner }}", "windows-2025",
            "ubuntu-24.04", "ubuntu-24.04", "${{ matrix.runner }}", "macos-15",
        ])
        self.assertEqual(re.findall(r"^        runner: (.+)$", WORKFLOW, re.M), [
            "[macos-15, ubuntu-24.04, ubuntu-24.04-arm]",
            "[windows-2025, windows-11-arm]",
            "[windows-2025, windows-11-arm]",
        ])
        self.assertEqual(re.findall(r"^          - runner: (.+)$", WORKFLOW, re.M),
                         ["ubuntu-24.04", "ubuntu-24.04-arm"])
