#!/usr/bin/env python3
"""Run one foreground command under a cooperative Unix resource lock."""
import argparse
import fcntl
import os
from pathlib import Path
import signal
import stat
import subprocess
import sys


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--lock', required=True, type=Path)
    parser.add_argument('command', nargs=argparse.REMAINDER)
    args = parser.parse_args()
    command = args.command
    if command[:1] == ['--']:
        command = command[1:]
    if not command:
        parser.error('a foreground command is required after --')
    args.lock.parent.mkdir(parents=True, exist_ok=True)
    # Keep the inode: deleting a lock file can permit a second independent lock.
    try:
        fd = os.open(args.lock, os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW | os.O_NONBLOCK, 0o600)
    except OSError as error:
        print(f'lock could not open: {error.strerror}', file=sys.stderr)
        return 2
    with os.fdopen(fd, 'a') as lock:
        if not stat.S_ISREG(os.fstat(lock.fileno()).st_mode):
            print('lock must be a regular file', file=sys.stderr)
            return 2
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            print('resource busy; command was not started', file=sys.stderr)
            return 75
        try:
            # The child retains the lease if this wrapper is abruptly killed.
            child = subprocess.Popen(command, pass_fds=(lock.fileno(),), start_new_session=True)
        except OSError as error:
            print(f'command could not start: {error.strerror}', file=sys.stderr)
            return 127

        def forward(signum, _frame):
            try:
                os.killpg(child.pid, signum)
            except ProcessLookupError:
                pass

        previous = {sig: signal.signal(sig, forward) for sig in (signal.SIGINT, signal.SIGTERM)}
        print(f'exclusive command started: pid={child.pid}', file=sys.stderr, flush=True)
        try:
            code = child.wait()
        finally:
            for sig, handler in previous.items():
                signal.signal(sig, handler)
        return code if code >= 0 else 128 - code


if __name__ == '__main__':
    sys.exit(main())
