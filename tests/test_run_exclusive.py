"""Behavior tests for the cooperative shared-resource runner."""
import os
from pathlib import Path
import re
import signal
import subprocess
import sys
import tempfile
import unittest

RUNNER = Path(__file__).resolve().parents[1] / 'codex/skills/taeguk-plan-cross-verification/scripts/run_exclusive.py'


class ExclusiveRunTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.lock = Path(self.tmp.name) / 'database.lock'

    def command(self, code, lock=None):
        return [sys.executable, str(RUNNER), '--lock', str(lock or self.lock), '--', sys.executable, '-c', code]

    def hold(self):
        process = subprocess.Popen(self.command('import time; time.sleep(30)'), stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True)
        line = process.stderr.readline()
        self.assertRegex(line, r'pid=\d+')
        child_pid = int(re.search(r'pid=(\d+)', line).group(1))

        def cleanup():
            try:
                os.killpg(child_pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
            if process.poll() is None:
                process.terminate()
            process.communicate(timeout=5)
        self.addCleanup(cleanup)
        return process, child_pid

    def run_once(self, code, lock=None):
        return subprocess.run(self.command(code, lock), capture_output=True, text=True, timeout=5)

    def test_busy_resource_does_not_start_command(self):
        self.hold()
        marker = Path(self.tmp.name) / 'should-not-exist'
        result = self.run_once(f'from pathlib import Path; Path({str(marker)!r}).touch()')
        self.assertEqual(result.returncode, 75)
        self.assertFalse(marker.exists())

    def test_distinct_resources_can_run(self):
        self.hold()
        self.assertEqual(self.run_once('pass', Path(self.tmp.name) / 'other.lock').returncode, 0)

    def test_failed_command_preserves_exit_and_releases_resource(self):
        self.assertEqual(self.run_once('raise SystemExit(17)').returncode, 17)
        self.assertEqual(self.run_once('pass').returncode, 0)

    def test_missing_command_releases_resource(self):
        result = subprocess.run([sys.executable, str(RUNNER), '--lock', str(self.lock), '--', '/nonexistent/taeguk-command'], capture_output=True, timeout=5)
        self.assertEqual(result.returncode, 127)
        self.assertEqual(self.run_once('pass').returncode, 0)

    def test_termination_is_forwarded_before_lease_release(self):
        process, _ = self.hold()
        process.terminate()
        process.communicate(timeout=5)
        self.assertEqual(process.returncode, 143)
        self.assertEqual(self.run_once('pass').returncode, 0)

    def test_symlink_lock_does_not_start_command(self):
        target = Path(self.tmp.name) / 'owned-file'
        target.write_text('preserve')
        self.lock.symlink_to(target)
        marker = Path(self.tmp.name) / 'should-not-exist'
        result = self.run_once(f'from pathlib import Path; Path({str(marker)!r}).touch()')
        self.assertEqual(result.returncode, 2)
        self.assertFalse(marker.exists())
        self.assertEqual(target.read_text(), 'preserve')

    def test_direct_child_keeps_lease_after_wrapper_is_killed(self):
        process, child_pid = self.hold()
        process.kill()
        process.wait(timeout=5)
        self.assertEqual(self.run_once('pass').returncode, 75)


if __name__ == '__main__':
    unittest.main()
