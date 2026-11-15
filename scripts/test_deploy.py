"""Tests for deploy.sh's setup, env and try --clean. Synthetic plists only.

They run deploy.sh against a throwaway plist in a temp dir with SUDO=''
and LAUNCHCTL=true, so nothing live is read or changed. macOS only
(PlistBuddy, plutil).
"""

import os
import plistlib
import shutil
import subprocess
import sys
import tempfile
import unittest

APP = "agent-avatar"
PREFIX = "AVATAR"
HERE = os.path.dirname(os.path.abspath(__file__))
DEPLOY = os.path.join(HERE, "deploy.sh")


def read(path):
    with open(path, "rb") as f:
        return f.read()


@unittest.skipUnless(sys.platform == "darwin" and os.path.exists("/usr/libexec/PlistBuddy"),
                     "needs macOS PlistBuddy")
class DeployEnv(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.mkdtemp()
        self.data = os.path.join(self.tmp, "data dir")  # a space, like Application Support
        os.makedirs(self.data)
        self.plist = os.path.join(self.tmp, f"com.{APP}.test.plist")
        self.write_plist({f"{PREFIX}_DATA_DIR": self.data, f"{PREFIX}_ADDR": "127.0.0.1:1"})
        self.build = os.path.join(self.tmp, "build")

    def tearDown(self):
        shutil.rmtree(self.tmp)

    def write_plist(self, env):
        with open(self.plist, "wb") as f:
            plistlib.dump({"Label": f"com.{APP}.test",
                           "ProgramArguments": ["/nonexistent/server"],
                           "EnvironmentVariables": env}, f)

    def env_vars(self):
        with open(self.plist, "rb") as f:
            return plistlib.load(f)["EnvironmentVariables"]

    def run_deploy(self, *args):
        env = dict(os.environ, SUDO="", LAUNCHCTL="true", **{
            f"{PREFIX}_PLIST": self.plist, f"{PREFIX}_BUILD_DIR": self.build})
        return subprocess.run(["bash", DEPLOY, *args], env=env, stdin=subprocess.DEVNULL,
                              capture_output=True, text=True)

    def backups(self):
        d = os.path.join(self.data, "plist-backups")
        return sorted(os.listdir(d)) if os.path.isdir(d) else []

    def test_set_adds_and_replaces_with_backup(self):
        before = read(self.plist)
        r = self.run_deploy("env", "set", "--yes", f"{PREFIX}_SCREENS=/a path/screens.toml",
                            f"{PREFIX}_ADDR=127.0.0.1:2")
        self.assertEqual(r.returncode, 0, r.stderr)
        v = self.env_vars()
        self.assertEqual(v[f"{PREFIX}_SCREENS"], "/a path/screens.toml")
        self.assertEqual(v[f"{PREFIX}_ADDR"], "127.0.0.1:2")
        self.assertIn("+", r.stdout)  # the diff was shown
        [b] = self.backups()
        self.assertEqual(read(os.path.join(self.data, "plist-backups", b)), before)

    def test_unset_and_restore(self):
        r = self.run_deploy("env", "unset", "--yes", f"{PREFIX}_ADDR")
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertNotIn(f"{PREFIX}_ADDR", self.env_vars())
        r = self.run_deploy("env", "restore", "--yes")
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertEqual(self.env_vars()[f"{PREFIX}_ADDR"], "127.0.0.1:1")

    def test_unset_missing_is_no_change(self):
        r = self.run_deploy("env", "unset", "--yes", "NOT_THERE")
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertIn("isn't set", r.stdout)
        self.assertIn("no change", r.stdout)
        self.assertEqual(self.backups(), [])

    def assert_refused(self, *args, says):
        before = read(self.plist)
        r = self.run_deploy("env", *args)
        self.assertNotEqual(r.returncode, 0)
        self.assertIn(says, r.stderr)
        self.assertEqual(read(self.plist), before)
        self.assertEqual(self.backups(), [])

    def test_refuses_secrets_bad_names_and_values(self):
        self.assert_refused("set", "--yes", f"{PREFIX}_TOKEN=abc", says="_FILE")
        self.assert_refused("set", "--yes", "SOME_SECRET=abc", says="_FILE")
        self.assert_refused("set", "--yes", "lower=1", says="isn't an env var name")
        self.assert_refused("set", "--yes", 'X="q"', says="quotes")
        self.assert_refused("set", "--yes", "X=", says="empty value")
        self.assert_refused("set", "--yes", "NOEQUALS", says="KEY=VALUE")

    def test_needs_a_terminal_without_yes(self):
        self.assert_refused("set", "X=1", says="terminal")

    def test_list(self):
        r = self.run_deploy("env", "list")
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertIn(f"{PREFIX}_ADDR=127.0.0.1:1", r.stdout.splitlines())

    def test_verify_needs_releases_layout(self):
        r = self.run_deploy("verify")
        self.assertNotEqual(r.returncode, 0)
        self.assertIn("nothing is installed", r.stderr)

    def test_usage(self):
        self.assertEqual(self.run_deploy().returncode, 2)
        self.assertEqual(self.run_deploy("try").returncode, 2)
        self.assertEqual(self.run_deploy("verify", "--bogus").returncode, 2)

    def test_try_clean_leaves_unrelated_processes(self):
        os.makedirs(os.path.join(self.build, "try-data"))
        other = subprocess.Popen(["sleep", "30"])
        try:
            with open(os.path.join(self.build, "try.pid"), "w") as f:
                f.write(str(other.pid))
            r = self.run_deploy("try", "--clean")
            self.assertEqual(r.returncode, 0, r.stderr)
            self.assertFalse(os.path.exists(os.path.join(self.build, "try-data")))
            self.assertFalse(os.path.exists(os.path.join(self.build, "try.pid")))
            self.assertIsNone(other.poll())  # not ours, so not killed
        finally:
            other.kill()
            other.wait()


@unittest.skipUnless(sys.platform == "darwin" and os.path.exists("/usr/libexec/PlistBuddy"),
                     "needs macOS PlistBuddy")
class DeploySetup(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.mkdtemp()
        self.data = os.path.join(self.tmp, "data dir")
        self.plist = os.path.join(self.tmp, "com.agent-avatar.test.plist")

    def tearDown(self):
        shutil.rmtree(self.tmp)

    def run_setup(self, *args):
        env = dict(os.environ, SUDO="", LAUNCHCTL="true", AVATAR_PLIST=self.plist,
                   AVATAR_DATA_DIR=self.data, AVATAR_BUILD_DIR=os.path.join(self.tmp, "b"))
        return subprocess.run(["bash", DEPLOY, "setup", *args], env=env,
                              stdin=subprocess.DEVNULL, capture_output=True, text=True)

    def test_setup_writes_plist_token_and_screens(self):
        r = self.run_setup("--addr", "127.0.0.1:9999")
        self.assertEqual(r.returncode, 0, r.stderr)
        with open(self.plist, "rb") as f:
            p = plistlib.load(f)
        self.assertEqual(p["ProgramArguments"],
                         [os.path.join(self.data, "releases/current/agent-avatar-server")])
        env = p["EnvironmentVariables"]
        self.assertEqual(env["AVATAR_ADDR"], "127.0.0.1:9999")
        token = env["AVATAR_TOKEN_FILE"]
        self.assertEqual(os.stat(token).st_mode & 0o777, 0o600)
        self.assertEqual(len(read(token).strip()), 64)
        self.assertNotIn(read(token).strip().decode(), r.stdout + r.stderr)
        self.assertTrue(os.path.exists(env["AVATAR_SCREENS"]))
        self.assertEqual(os.stat(self.data).st_mode & 0o777, 0o700)

    def test_setup_keeps_token_and_refuses_twice(self):
        os.makedirs(self.data)
        with open(os.path.join(self.data, "token"), "w") as f:
            f.write("kept")
        self.assertEqual(self.run_setup().returncode, 0)
        self.assertEqual(read(os.path.join(self.data, "token")), b"kept")
        r = self.run_setup()
        self.assertNotEqual(r.returncode, 0)
        self.assertIn("exists", r.stderr)

    def test_setup_rejects_bad_addr(self):
        r = self.run_setup("--addr", "nope")
        self.assertNotEqual(r.returncode, 0)
        self.assertFalse(os.path.exists(self.plist))


@unittest.skipUnless(sys.platform == "darwin" and os.path.exists("/usr/libexec/PlistBuddy"),
                     "needs macOS PlistBuddy")
class DeploySetupMatrix(unittest.TestCase):
    """setup-matrix and env --matrix, after a plain setup. Synthetic ids."""

    ARGS = ("--homeserver", "http://127.0.0.1:8008", "--user", "@tablet:example.org",
            "--room", "!room:example.org", "--agent", "@agent:example.org")

    def setUp(self):
        self.tmp = tempfile.mkdtemp()
        self.data = os.path.join(self.tmp, "data dir")
        self.plist = os.path.join(self.tmp, "com.agent-avatar.test.plist")
        self.mplist = os.path.join(self.tmp, "com.agent-avatar.test-matrix.plist")
        r = self.run_deploy("setup", "--addr", "127.0.0.1:9999")
        self.assertEqual(r.returncode, 0, r.stderr)
        self.tokf = os.path.join(self.data, "matrix-token")

    def tearDown(self):
        shutil.rmtree(self.tmp)

    def run_deploy(self, *args):
        env = dict(os.environ, SUDO="", LAUNCHCTL="false", AVATAR_PLIST=self.plist,
                   AVATAR_MATRIX_PLIST=self.mplist, AVATAR_DATA_DIR=self.data,
                   AVATAR_BUILD_DIR=os.path.join(self.tmp, "b"))
        return subprocess.run(["bash", DEPLOY, *args], env=env,
                              stdin=subprocess.DEVNULL, capture_output=True, text=True)

    def write_token(self, mode=0o600):
        with open(self.tokf, "w") as f:
            f.write("synthetic-matrix-token")
        os.chmod(self.tokf, mode)

    def test_writes_the_adapter_plist(self):
        self.write_token()
        r = self.run_deploy("setup-matrix", *self.ARGS)
        self.assertEqual(r.returncode, 0, r.stderr)
        with open(self.mplist, "rb") as f:
            p = plistlib.load(f)
        self.assertEqual(p["Label"], "com.agent-avatar.matrix")
        self.assertEqual(p["ProgramArguments"],
                         [os.path.join(self.data, "releases/current/agent-avatar-matrix")])
        env = p["EnvironmentVariables"]
        self.assertEqual(env["AVATAR_URL"], "http://127.0.0.1:9999")
        self.assertEqual(env["AVATAR_TOKEN_FILE"], os.path.join(self.data, "token"))
        self.assertEqual(env["MATRIX_TOKEN_FILE"], self.tokf)
        self.assertEqual(env["MATRIX_ROOM"], "!room:example.org")
        self.assertEqual(env["MATRIX_AGENT_USER"], "@agent:example.org")
        self.assertNotIn("synthetic-matrix-token", read(self.mplist).decode())
        # No release is installed, so nothing was started.
        self.assertIn("starts with the next install", r.stdout)
        r = self.run_deploy("setup-matrix", *self.ARGS)
        self.assertIn("exists", r.stderr)

    def test_needs_a_0600_token_file(self):
        r = self.run_deploy("setup-matrix", *self.ARGS)
        self.assertIn("matrix-token", r.stderr)
        self.write_token(0o644)
        r = self.run_deploy("setup-matrix", *self.ARGS)
        self.assertIn("0600", r.stderr)
        self.assertFalse(os.path.exists(self.mplist))

    def test_rejects_malformed_ids(self):
        self.write_token()
        for i, bad in [(5, "#alias:example.org"), (3, "tablet"), (7, "!room:example.org"),
                       (1, "127.0.0.1:8008")]:
            args = list(self.ARGS)
            args[i] = bad
            r = self.run_deploy("setup-matrix", *args)
            self.assertNotEqual(r.returncode, 0, bad)
        self.assertFalse(os.path.exists(self.mplist))

    def test_env_matrix_edits_the_adapter_plist(self):
        r = self.run_deploy("env", "--matrix", "list")
        self.assertIn("setup-matrix", r.stderr)
        self.write_token()
        self.assertEqual(self.run_deploy("setup-matrix", *self.ARGS).returncode, 0)
        r = self.run_deploy("env", "--matrix", "set", "--yes", "MATRIX_SETTLE_MS=2500")
        self.assertEqual(r.returncode, 0, r.stderr)
        with open(self.mplist, "rb") as f:
            self.assertEqual(plistlib.load(f)["EnvironmentVariables"]["MATRIX_SETTLE_MS"], "2500")
        with open(self.plist, "rb") as f:
            self.assertNotIn("MATRIX_SETTLE_MS", plistlib.load(f)["EnvironmentVariables"])
        r = self.run_deploy("env", "--matrix", "set", "--yes", "MATRIX_TOKEN=x")
        self.assertIn("_FILE", r.stderr)
        r = self.run_deploy("status")
        self.assertIn("adapter: not loaded", r.stdout)


if __name__ == "__main__":
    unittest.main()
