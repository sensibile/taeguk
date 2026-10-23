"""Behavior tests for the cooperative shared-resource runner."""
import os
from pathlib import Path
import re
import signal
import stat
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

    def run_with_signal_around_spawn(self, signum, after_spawn, code):
        """Deliver a signal to the wrapper as Popen starts or right after it returns."""
        pid_file = Path(self.tmp.name) / 'child.pid'
        harness = f'''
import importlib.util, os, signal, subprocess, sys
spec = importlib.util.spec_from_file_location('run_exclusive', {str(RUNNER)!r})
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
real_popen = subprocess.Popen

def popen(*args, **kwargs):
    if not {after_spawn!r}:
        os.kill(os.getpid(), {int(signum)})
    child = real_popen(*args, **kwargs)
    with open({str(pid_file)!r}, 'w') as handle:
        handle.write(str(child.pid))
    if {after_spawn!r}:
        os.kill(os.getpid(), {int(signum)})
    return child

module.subprocess.Popen = popen
sys.argv = ['run_exclusive.py', '--lock', {str(self.lock)!r}, '--', sys.executable, '-c', {code!r}]
sys.exit(module.main())
'''

        def cleanup():
            if pid_file.exists():
                try:
                    os.killpg(int(pid_file.read_text()), signal.SIGKILL)
                except (ProcessLookupError, PermissionError):
                    pass
        self.addCleanup(cleanup)
        # A leaked child must not hold pipes open and turn the failure into a timeout.
        return subprocess.run([sys.executable, '-c', harness], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=10)

    def test_signal_right_after_spawn_is_forwarded(self):
        result = self.run_with_signal_around_spawn(signal.SIGTERM, True, 'import time; time.sleep(30)')
        self.assertEqual(result.returncode, 143)
        self.assertEqual(self.run_once('pass').returncode, 0)

    def test_signal_during_spawn_is_forwarded(self):
        result = self.run_with_signal_around_spawn(signal.SIGTERM, False, 'import time; time.sleep(30)')
        self.assertEqual(result.returncode, 143)
        self.assertEqual(self.run_once('pass').returncode, 0)

    def run_patched(self, patch, code):
        """Run the wrapper in-process after `patch` replaces part of its environment."""
        harness = f'''
import importlib.util, os, signal, subprocess, sys
spec = importlib.util.spec_from_file_location('run_exclusive', {str(RUNNER)!r})
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
{patch}
sys.argv = ['run_exclusive.py', '--lock', {str(self.lock)!r}, '--', sys.executable, '-c', {code!r}]
sys.exit(module.main())
'''
        return subprocess.run([sys.executable, '-c', harness], capture_output=True, text=True, timeout=10)

    def test_signal_queued_before_failed_spawn_is_reported(self):
        # A start failure must not swallow a stop request that already arrived.
        patch = f'''
def popen(*args, **kwargs):
    os.kill(os.getpid(), {int(signal.SIGTERM)})
    raise FileNotFoundError(2, 'No such file or directory')
module.subprocess.Popen = popen
'''
        result = self.run_patched(patch, 'pass')
        self.assertEqual(result.returncode, 143, result.stderr)
        self.assertIn('interrupted', result.stderr)
        self.assertEqual(self.run_once('pass').returncode, 0)

    def test_signal_during_handler_installation_is_not_lost(self):
        # SIGTERM arrives after the first handler is installed but before its own.
        marker = Path(self.tmp.name) / 'should-not-exist'
        patch = f'''
real_signal = signal.signal
installed = []
def install(sig, handler):
    previous = real_signal(sig, handler)
    if not installed:
        os.kill(os.getpid(), {int(signal.SIGTERM)})
    installed.append(sig)
    return previous
module.signal.signal = install
'''
        result = self.run_patched(patch, f'from pathlib import Path; Path({str(marker)!r}).touch()')
        self.assertEqual(result.returncode, 143, result.stderr)
        self.assertFalse(marker.exists())
        self.assertEqual(self.run_once('pass').returncode, 0)

    def assert_signal_forwarded_before_lease_release(self, signum):
        process, _ = self.hold()
        process.send_signal(signum)
        # wait(), not communicate(): a leaked child keeps the stderr pipe open.
        process.wait(timeout=5)
        self.assertEqual(process.returncode, 128 + signum)
        self.assertEqual(self.run_once('pass').returncode, 0)

    def test_hangup_is_forwarded_before_lease_release(self):
        self.assert_signal_forwarded_before_lease_release(signal.SIGHUP)

    def test_quit_is_forwarded_before_lease_release(self):
        self.assert_signal_forwarded_before_lease_release(signal.SIGQUIT)

    def test_missing_lock_directories_are_private_even_with_permissive_umask(self):
        # Another account must not be able to pre-create the lock in a directory the runner made.
        lock = Path(self.tmp.name) / 'state' / 'taeguk' / 'locks' / 'database.lock'
        result = subprocess.run(['sh', '-c', 'umask 000 && exec "$@"', 'sh', *self.command('pass', lock)], capture_output=True, text=True, timeout=5)
        self.assertEqual(result.returncode, 0, result.stderr)
        for directory in (lock.parent, lock.parent.parent, lock.parent.parent.parent):
            with self.subTest(str(directory.relative_to(self.tmp.name))):
                self.assertEqual(stat.S_IMODE(directory.stat().st_mode), 0o700)

    def test_restrictive_umask_still_creates_a_reusable_private_lock(self):
        # umask 0777 strips owner bits too: creation must not depend on a later chmod.
        lock = Path(self.tmp.name) / 'state' / 'locks' / 'database.lock'
        umask_run = lambda: subprocess.run(['sh', '-c', 'umask 0777 && exec "$@"', 'sh', *self.command('pass', lock)], capture_output=True, text=True, timeout=5)
        for attempt in ('first', 'second'):
            with self.subTest(attempt):
                result = umask_run()
                self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(stat.S_IMODE(lock.parent.stat().st_mode), 0o700)
        self.assertEqual(stat.S_IMODE(lock.stat().st_mode), 0o600)

    def test_command_keeps_the_callers_umask(self):
        code = 'import os; current = os.umask(0); print(oct(current))'
        result = subprocess.run(['sh', '-c', 'umask 027 && exec "$@"', 'sh', *self.command(code)], capture_output=True, text=True, timeout=5)
        self.assertEqual(result.stdout.strip(), '0o27', result.stderr)

    def test_existing_shared_lock_keeps_its_mode(self):
        # The shared-group setup relies on a pre-created group-writable lock staying that way.
        self.lock.touch()
        self.lock.chmod(0o660)
        self.assertEqual(self.run_once('pass').returncode, 0)
        self.assertEqual(stat.S_IMODE(self.lock.stat().st_mode), 0o660)

    @unittest.skipIf(os.geteuid() == 0, 'root ignores file permissions')
    def test_lock_another_account_created_first_blocks_the_command(self):
        # Why the guide keeps locks out of shared /tmp: a pre-created lock the
        # runner cannot open stops every run (exit 2) without starting it.
        self.lock.touch(mode=0o000)
        self.addCleanup(self.lock.chmod, 0o600)
        marker = Path(self.tmp.name) / 'should-not-exist'
        result = self.run_once(f'from pathlib import Path; Path({str(marker)!r}).touch()')
        self.assertEqual(result.returncode, 2)
        self.assertIn('lock could not open', result.stderr)
        self.assertFalse(marker.exists())

    def test_direct_child_keeps_lease_after_wrapper_is_killed(self):
        process, child_pid = self.hold()
        process.kill()
        process.wait(timeout=5)
        self.assertEqual(self.run_once('pass').returncode, 75)


if __name__ == '__main__':
    unittest.main()
