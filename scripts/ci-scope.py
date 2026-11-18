#!/usr/bin/env python3
"""Choose what a CI run checks: every platform, or Linux and the changed crates.

A pull request runs on Linux alone and lints and tests only the crates its
changes can reach: each changed crate and every crate that depends on it, by
any dependency kind. Any other build input (`assets/`, `scripts/`, `skills/`,
`crates/test-support`) reaches every crate. The merge queue, main, and pull
requests that touch platform code or dependencies, or ask for it with a
label, run every platform over the whole workspace, so a release still
requires the full matrix on main (scripts/release/check-ci.py).

Prints GitHub step outputs:
  full           `true` for every platform and the whole workspace
  cargo_scope    `--workspace`, `-p NAME ...`, or empty when no crate changed
  gpui           `true` when sheprd-gpui is in scope
"""
import json
import os
import re
import subprocess
import sys
import tomllib
from collections import deque
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
# The existing platform labels, and one to ask for every platform outright.
FULL_LABELS = frozenset({"full-ci", "macos", "windows"})
# A change to how CI picks its work has to prove itself on every platform.
FULL_PATHS = frozenset({".github/workflows/ci.yml", "scripts/ci-changes.sh", "scripts/ci-scope.py"})
# What each platform builds: dependencies (a manifest's target tables, and
# crates only one platform compiles, change with the lockfile) and the
# toolchain.
BUILD_PATH = re.compile(r"(^|/)Cargo\.toml$|^Cargo\.lock$|^rust-toolchain\.toml$|^\.cargo/")
# Code only a Windows or macOS runner compiles or runs: platform files,
# PowerShell, cfg gates, and build scripts reading the target.
PLATFORM_PATH = re.compile(r"(^|[/_.-])(windows|win32|macos|darwin)([/_.-]|$)|\.ps1$", re.IGNORECASE)
PLATFORM_CFG = re.compile(
    r"\bcfg(!|_attr)?\(.*\b(windows|macos|target_os|target_vendor|target_family|unix)\b"
    r"|CARGO_CFG_TARGET_(OS|VENDOR|FAMILY)\b")
# Outside a crate, what no crate builds or tests read: Markdown but the
# skills the app embeds, the website but the relay list sheprd-relay's tests
# check, the hosted site, and other workflows. Anything else there reaches
# every crate.
UNREAD = re.compile(r"^(?!skills/).*\.md$|^website/(?!site/relays\.json)|^(site|\.github)/")
# Patched into GPUI's build, which no member manifest names.
WORKSPACE_WIDE_CRATES = frozenset({"crates/embed-resource"})
DEPENDENCY_TABLES = ("dependencies", "dev-dependencies", "build-dependencies")


def workspace(root=ROOT):
    """Returns ({member directory: package name}, {package: dependent packages})."""
    manifest = tomllib.loads((root / "Cargo.toml").read_text())
    members = {}
    for pattern in manifest["workspace"]["members"]:
        for directory in sorted(root.glob(pattern)):
            package = tomllib.loads((directory / "Cargo.toml").read_text())["package"]
            members[directory.relative_to(root).as_posix()] = package["name"]
    names = set(members.values())
    dependents = {name: set() for name in names}
    for directory, name in members.items():
        package = tomllib.loads((root / directory / "Cargo.toml").read_text())
        tables = [package, *package.get("target", {}).values()]
        for table in tables:
            for kind in DEPENDENCY_TABLES:
                for key, value in table.get(kind, {}).items():
                    dependency = value.get("package", key) if isinstance(value, dict) else key
                    if dependency in names:
                        dependents[dependency].add(name)
    return members, dependents


def reach(seeds, dependents):
    """Every package in seeds and every package that depends on one of them."""
    reached = set(seeds)
    queue = deque(seeds)
    while queue:
        for dependent in dependents[queue.popleft()]:
            if dependent not in reached:
                reached.add(dependent)
                queue.append(dependent)
    return reached


def owner(path, members):
    for directory, name in members.items():
        if path.startswith(directory + "/"):
            return name
    return None


def plan(event, labels, changed, diff_lines, members, dependents):
    """The step outputs. `changed` is None when the run has no usable base."""
    full = (event != "pull_request" or changed is None
            or bool(FULL_LABELS & set(labels))
            or any(path in FULL_PATHS or BUILD_PATH.search(path) or PLATFORM_PATH.search(path)
                   for path in changed)
            or any(PLATFORM_CFG.search(line) for line in diff_lines))
    if full:
        return {"full": "true", "cargo_scope": "--workspace", "gpui": "true"}
    seeds = set()
    everything = False
    for path in changed:
        if any(path.startswith(directory + "/") for directory in WORKSPACE_WIDE_CRATES):
            everything = True
        elif name := owner(path, members):
            seeds.add(name)
        elif not UNREAD.search(path):
            everything = True
    scope = set(dependents) if everything else reach(seeds, dependents)
    cargo_scope = "--workspace" if everything else " ".join(f"-p {name}" for name in sorted(scope))
    return {"full": "false", "cargo_scope": cargo_scope, "gpui": str("sheprd-gpui" in scope).lower()}


def git(*args):
    return subprocess.run(["git", *args], cwd=ROOT, check=True, text=True,
                          stdout=subprocess.PIPE).stdout


def changes(event, base, head):
    """Changed paths, both sides of a rename, and changed Rust lines, or None."""
    if not re.fullmatch(r"[0-9a-f]{40}", base or ""):
        return None, []
    if subprocess.run(["git", "cat-file", "-e", f"{base}^{{commit}}"], cwd=ROOT,
                      stderr=subprocess.DEVNULL).returncode != 0:
        return None, []
    # Pull requests use their merge base, as scripts/ci-changes.sh does.
    if event == "pull_request":
        base = git("merge-base", base, head).strip()
    paths = git("diff", "--name-only", "--no-renames", base, head).splitlines()
    lines = [line for line in git("diff", "-U0", "--no-renames", base, head, "--", "*.rs").splitlines()
             if line[:1] in "+-" and not line.startswith(("+++", "---"))]
    return paths, lines


def main():
    event = os.environ["EVENT"]
    labels = json.loads(os.environ.get("LABELS") or "[]") or []
    changed, diff_lines = changes(event, os.environ.get("BASE", ""), os.environ["HEAD_SHA"])
    members, dependents = workspace()
    for key, value in plan(event, labels, changed, diff_lines, members, dependents).items():
        print(f"{key}={value}")


if __name__ == "__main__":
    try:
        main()
    except (KeyError, subprocess.CalledProcessError) as error:
        sys.exit(f"ci-scope: {error!r}")
