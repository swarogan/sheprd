"""The push relay's deploy files: the droplet's start.sh takes only secrets
for its own APNs environment and replaces them only once they check out,
runs only a verified image, and make-secrets.sh writes what the relay
reads."""

import json
import os
import re
import shutil
import signal
import stat
import subprocess
import tempfile
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DEPLOY = os.path.join(ROOT, "deploy", "push-relay")
SECRETS = ("apns/config.json", "apns/AuthKey.p8", "relay-key.pem", "credential-secret")
IDENTITY = (
    "https://github.com/sheprd-labs/sheprd/.github/workflows/push-relay-image.yml@refs/heads/main"
)


def read(name: str) -> str:
    with open(os.path.join(DEPLOY, name), encoding="utf-8") as file:
        return file.read()


def rendered(environment: str) -> str:
    """cloud-init.yml after create-droplet.sh's sed substitution."""
    return (
        read("cloud-init.yml")
        .replace("__COMPOSE_B64__", "Y29tcG9zZQ==")
        .replace("__CADDYFILE_B64__", "Y2FkZHk=")
        .replace("__PUSH_RELAY_DOMAIN__", "push.example.org")
        .replace("__PUSH_RELAY_ENVIRONMENT__", environment)
    )


def functions(*names: str) -> str:
    """start.sh's shell functions NAMES, unindented, to run on their own."""
    text = rendered("production")
    out = []
    for name in names:
        block = re.search(rf"\n(\s*){name}\(\) \{{\n.*?\n\1\}}\n", text, re.S)
        assert block, f"start.sh's {name} is missing"
        out.append("\n".join(line.strip() for line in block.group(0).splitlines()))
    return "\n".join(out)


def write_secrets(directory: str, sandbox: bool, marker: str = "") -> None:
    os.makedirs(os.path.join(directory, "apns"))
    for name in SECRETS:
        with open(os.path.join(directory, name), "w", encoding="utf-8") as file:
            if name == "apns/config.json":
                json.dump({"sandbox": sandbox}, file)
            else:
                file.write(marker)


def shell(directory: str, script: str, environment: str = "production",
          stdin: bytes = b"") -> subprocess.CompletedProcess:
    """Runs SCRIPT after start.sh's secrets functions, in DIRECTORY, as the
    relay for ENVIRONMENT."""
    body = f'environment="{environment}"\n'
    body += functions("check_secrets", "stage_secrets", "swap_secrets", "restore_secrets",
                      "refuse_new")
    body += "\n" + script + "\n"
    return subprocess.run(["sh", "-c", body], cwd=directory, capture_output=True,
                          input=stdin, env={"PATH": os.environ["PATH"]})


def marker(directory: str, name: str = "secrets") -> str:
    with open(os.path.join(directory, name, "relay-key.pem"), encoding="utf-8") as file:
        return file.read()


def tar_of(directory: str) -> bytes:
    return subprocess.run(["tar", "-C", directory, "-cf", "-", *SECRETS], check=True,
                          capture_output=True, env={"PATH": os.environ["PATH"],
                                                    "COPYFILE_DISABLE": "1"}).stdout


@unittest.skipUnless(shutil.which("jq"), "jq is not installed")
class SecretsTest(unittest.TestCase):
    def test_secrets_for_this_environment_pass(self) -> None:
        for environment, sandbox in (("production", False), ("sandbox", True)):
            with self.subTest(environment=environment), tempfile.TemporaryDirectory() as d:
                write_secrets(os.path.join(d, "secrets"), sandbox)
                result = shell(d, "check_secrets secrets", environment)
                self.assertEqual(result.returncode, 0, result.stderr)

    def test_a_key_for_the_other_environment_is_refused(self) -> None:
        with tempfile.TemporaryDirectory() as d:
            write_secrets(os.path.join(d, "secrets"), sandbox=True)
            result = shell(d, "check_secrets secrets")
            self.assertNotEqual(result.returncode, 0)
            self.assertIn(b"for the other APNs environment", result.stderr)

    def test_a_missing_secret_is_refused(self) -> None:
        with tempfile.TemporaryDirectory() as d:
            write_secrets(os.path.join(d, "secrets"), sandbox=False)
            os.remove(os.path.join(d, "secrets", "credential-secret"))
            result = shell(d, "check_secrets secrets")
            self.assertNotEqual(result.returncode, 0)
            self.assertIn(b"credential-secret", result.stderr)

    def test_new_secrets_arrive_on_stdin_into_staging(self) -> None:
        with tempfile.TemporaryDirectory() as source, tempfile.TemporaryDirectory() as d:
            write_secrets(source, sandbox=False, marker="new")
            write_secrets(os.path.join(d, "secrets"), sandbox=False, marker="old")
            result = shell(d, "stage_secrets && check_secrets secrets.new", stdin=tar_of(source))
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(marker(d, "secrets.new"), "new")
            self.assertEqual(marker(d), "old")

    def test_refused_new_secrets_leave_the_old_in_place(self) -> None:
        with tempfile.TemporaryDirectory() as d:
            write_secrets(os.path.join(d, "secrets"), sandbox=False, marker="old")
            write_secrets(os.path.join(d, "secrets.new"), sandbox=True, marker="new")
            result = shell(d, "check_secrets secrets.new || refuse_new")
            self.assertNotEqual(result.returncode, 0)
            self.assertIn(b"unchanged", result.stderr)
            self.assertEqual(marker(d), "old")
            self.assertEqual(sorted(os.listdir(d)), ["secrets"])

    def test_the_old_secrets_wait_until_the_relay_answers(self) -> None:
        with tempfile.TemporaryDirectory() as d:
            write_secrets(os.path.join(d, "secrets"), sandbox=False, marker="old")
            write_secrets(os.path.join(d, "secrets.new"), sandbox=False, marker="new")
            result = shell(d, "swap_secrets")
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(marker(d), "new")
            self.assertEqual(marker(d, "secrets.old"), "old")
            result = shell(d, "restore_secrets")
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(marker(d), "old")
            self.assertEqual(sorted(os.listdir(d)), ["secrets"])

    def test_the_first_secrets_need_no_old_ones(self) -> None:
        with tempfile.TemporaryDirectory() as d:
            write_secrets(os.path.join(d, "secrets.new"), sandbox=True, marker="new")
            result = shell(d, "swap_secrets", "sandbox")
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(marker(d), "new")


@unittest.skipUnless(shutil.which("flock"), "flock is not installed")
class LockTest(unittest.TestCase):
    def test_a_second_run_refuses_while_one_holds_the_lock(self) -> None:
        with tempfile.TemporaryDirectory() as d:
            take = functions("take_lock") + "\ntake_lock\n"
            # Its own process group, so the sleep holding the lock's
            # descriptor goes with it.
            holder = subprocess.Popen(["sh", "-c", take + "echo held; sleep 30"], cwd=d,
                                      stdout=subprocess.PIPE, start_new_session=True)
            try:
                self.assertEqual(holder.stdout.readline(), b"held\n")
                second = subprocess.run(["sh", "-c", take], cwd=d, capture_output=True)
                self.assertNotEqual(second.returncode, 0)
                self.assertIn(b"running already", second.stderr)
            finally:
                os.killpg(holder.pid, signal.SIGKILL)
                holder.wait()
                holder.stdout.close()
            again = subprocess.run(["sh", "-c", take], cwd=d, capture_output=True)
            self.assertEqual(again.returncode, 0, again.stderr)


class StartTest(unittest.TestCase):
    def test_create_droplet_fills_every_placeholder(self) -> None:
        placeholders = set(re.findall(r"__[A-Z0-9_]+__", read("cloud-init.yml")))
        filled = set(re.findall(r"s\|(__[A-Z0-9_]+__)\|", read("create-droplet.sh")))
        self.assertEqual(placeholders, filled)

    def test_only_this_workflows_signature_is_accepted(self) -> None:
        text = rendered("production")
        self.assertIn(f"--certificate-identity {IDENTITY}", text)
        self.assertIn("ghcr.io/sheprd-labs/sheprd-push-relay:latest", text)

    def test_the_order_of_a_run(self) -> None:
        text = rendered("production")
        steps = [
            "take_lock || exit 1",
            "stage_secrets\n",
            "check_secrets secrets.new || refuse_new",
            "if ! image=$(verified_image); then",
            "--secrets /secrets --check",
            "swap_secrets\n",
            "up -d --force-recreate relay",
            "if ! relay_answers; then",
            "rm -rf secrets.old\n      echo",
        ]
        at = [text.index(step, text.index("take_lock || exit 1")) for step in steps]
        self.assertEqual(at, sorted(at), steps)

    def test_new_secrets_are_checked_by_the_relay_in_the_image_about_to_run(self) -> None:
        text = rendered("production")
        run = text[text.index("docker run --rm"):text.index("|| refuse_new\n        swap_secrets")]
        self.assertIn('"$image" --secrets /secrets --check', run)
        self.assertIn("--network none", run)
        self.assertIn('secrets.new:/secrets:ro', run)

    def test_cosign_matches_the_relays_pinned_hashes(self) -> None:
        pattern = r"(amd64|arm64)\) cosign_sha256=([0-9a-f]{64}) ;;"
        with open(os.path.join(ROOT, "deploy", "relay", "cloud-init.yml"), encoding="utf-8") as file:
            relay = set(re.findall(pattern, file.read()))
        self.assertEqual(set(re.findall(pattern, rendered("production"))), relay)
        self.assertEqual({arch for arch, _ in relay}, {"amd64", "arm64"})

    def test_no_secret_goes_in_user_data(self) -> None:
        paths = re.findall(r"- path: (\S+)", read("cloud-init.yml"))
        self.assertTrue(paths)
        self.assertFalse([path for path in paths if "/secrets" in path], paths)


class ComposeTest(unittest.TestCase):
    def relay(self) -> str:
        text = read("compose.yml")
        return text[text.index("  relay:\n") : text.index("  caddy:\n")]

    def test_only_caddy_reaches_the_relay(self) -> None:
        relay = self.relay()
        self.assertNotRegex(relay, r"\n\s*ports:")
        self.assertIn("--behind-proxy", relay)

    def test_the_relay_runs_locked_down_with_secrets_read_only(self) -> None:
        relay = self.relay()
        self.assertIn("./secrets:/secrets:ro", relay)
        self.assertIn("read_only: true", relay)
        self.assertIn("cap_drop: [ALL]", relay)
        self.assertIn("sqlite:/data/", relay)


@unittest.skipUnless(shutil.which("openssl"), "openssl is not installed")
class MakeSecretsTest(unittest.TestCase):
    def make(self, directory: str, *arguments: str) -> subprocess.CompletedProcess:
        key = os.path.join(directory, "AuthKey_XYZ9876543.p8")
        if not os.path.exists(key):
            subprocess.run(
                ["openssl", "genpkey", "-algorithm", "EC", "-pkeyopt",
                 "ec_paramgen_curve:P-256", "-out", key],
                check=True, capture_output=True,
            )
        return subprocess.run(
            [os.path.join(DEPLOY, "make-secrets.sh"), arguments[0], key, *arguments[1:]],
            capture_output=True, text=True,
        )

    def test_it_writes_what_the_relay_reads_private_to_this_user(self) -> None:
        with tempfile.TemporaryDirectory() as d:
            out = os.path.join(d, "sandbox")
            result = self.make(d, "sandbox", "ABCDE12345", "XYZ9876543", out)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn("BEGIN PUBLIC KEY", result.stdout)
            for name in SECRETS:
                mode = stat.S_IMODE(os.stat(os.path.join(out, name)).st_mode)
                self.assertEqual(mode & 0o077, 0, name)
            with open(os.path.join(out, "apns", "config.json"), encoding="utf-8") as file:
                self.assertEqual(
                    json.load(file),
                    {"team_id": "ABCDE12345", "key_id": "XYZ9876543",
                     "topic": "so.pen.herdr.remote", "sandbox": True},
                )
            with open(os.path.join(out, "credential-secret"), encoding="utf-8") as file:
                self.assertRegex(file.read().strip(), r"^[0-9a-f]{64}$")
            with open(os.path.join(out, "relay-key.pem"), encoding="utf-8") as file:
                self.assertIn("BEGIN PRIVATE KEY", file.read())

    def test_it_never_replaces_existing_secrets(self) -> None:
        with tempfile.TemporaryDirectory() as d:
            result = self.make(d, "production", "ABCDE12345", "XYZ9876543", d)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("exists", result.stderr)

    def test_it_refuses_malformed_ids(self) -> None:
        with tempfile.TemporaryDirectory() as d:
            result = self.make(d, "production", "abc", "XYZ9876543", os.path.join(d, "out"))
            self.assertNotEqual(result.returncode, 0)
            self.assertFalse(os.path.exists(os.path.join(d, "out")))


if __name__ == "__main__":
    unittest.main()
