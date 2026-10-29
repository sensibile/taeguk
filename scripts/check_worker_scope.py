#!/usr/bin/env python3
"""Check that a scheduled worker PR changes only what its worker may change, before auto-merge.

This checks paths, file modes and, for Rust sources, that test-gap edits stay inside the trailing
`#[cfg(test)]` module. It does not judge whether a change is needed or correct.
"""
import argparse
import json
import subprocess
import sys

WORKERS = ('test-gap', 'daily-cleanup')

# Never changed by a worker PR, whatever the worker. Matched case-insensitively, since a
# case-insensitive checkout puts `Docs/Rules/x.md` next to the real rules.
PROTECTED_PREFIXES = (
    '.github/', '.taeguk/', '.claude/', '.codex/', '.agents/',
    'codex/',        # skills and agent roles: agent instructions
    'docs/rules/',   # rule bodies, registry, approvals, proposals
)
PROTECTED_NAMES = {
    'agents.md', 'claude.md', 'claude.local.md',
    '.gitattributes', '.gitignore', '.gitmodules',
    'cargo.toml', 'cargo.lock', 'mix.exs', 'mix.lock',
}
PROTECTED_FILES = {
    'scripts/check_worker_scope.py', 'tests/test_check_worker_scope.py',
    'scripts/check_rules.py', 'tests/test_rules.py',
    'docs/repository-operations.md',
}

TEST_PREFIXES = ('tests/', 'apps/taeguk/test/', 'tools/taeguk-cli/tests/')
RUST_SOURCE_PREFIX = 'tools/taeguk-cli/src/'
CLEANUP_PREFIXES = TEST_PREFIXES + (
    'scripts/', 'apps/taeguk/lib/', RUST_SOURCE_PREFIX, 'docs/',
)
CLEANUP_FILES = {'README.md'}

REGULAR_MODES = {'100644', '100755'}
ZERO = '0' * 40


def git(*args, check=True):
    out = subprocess.run(['git', *args], capture_output=True)
    if check and out.returncode != 0:
        raise RuntimeError(f'git {args[0]} failed: ' + out.stderr.decode(errors='replace').strip())
    return out


def changes(base, head):
    """Raw entries from the merge-base of base and head to head, without rename detection."""
    raw = git('diff', '--raw', '-z', '--no-renames', '--no-ext-diff', '--no-textconv', '--abbrev=40',
              f'{base}...{head}').stdout
    parts = raw.split(b'\0')
    entries = []
    i = 0
    while i + 1 < len(parts) and parts[i]:
        meta = parts[i].decode('ascii').lstrip(':').split()
        path = parts[i+1].decode('utf-8', errors='surrogateescape')
        entries.append({'old_mode': meta[0], 'new_mode': meta[1], 'old': meta[2], 'new': meta[3],
                        'status': meta[4], 'path': path})
        i += 2
    return entries


def blob(sha):
    return b'' if sha == ZERO else git('cat-file', 'blob', sha).stdout


def protected(path):
    lower = path.lower()
    name = lower.rsplit('/', 1)[-1]
    return (lower.startswith(PROTECTED_PREFIXES) or name in PROTECTED_NAMES
            or lower in PROTECTED_FILES or (name.startswith('agents') and name.endswith('.md')))


def test_module_start(lines):
    """Index of the `#[cfg(test)]` line opening the trailing test module, or None.

    The module must be the last item: from its `mod ... {` line the brace depth, counted outside
    strings, chars and comments, returns to zero only at the last non-blank line.
    """
    starts = [i for i, line in enumerate(lines) if line.strip() == '#[cfg(test)]']
    if len(starts) != 1:
        return None
    start = starts[0]
    if start + 1 >= len(lines) or not lines[start+1].strip().startswith('mod ') or not lines[start+1].rstrip().endswith('{'):
        return None
    last = max((i for i, line in enumerate(lines) if line.strip()), default=-1)
    depth = 0
    for i in range(start + 1, len(lines)):
        depth += brace_delta(lines[i])
        if depth < 0:
            return None
        if depth == 0:
            return start if i == last else None
    return None


def brace_delta(line):
    """Net braces in a line outside string and char literals and line comments."""
    delta, i, quote = 0, 0, None
    while i < len(line):
        c = line[i]
        if quote:
            if c == '\\':
                i += 1
            elif c == quote:
                quote = None
        elif c == '/' and line[i:i+2] == '//':
            break
        elif c == '"':
            quote = '"'
        elif c == "'" and (line[i+2:i+3] == "'" or line[i+1:i+2] == '\\'):
            quote = "'"
        elif c == '{':
            delta += 1
        elif c == '}':
            delta -= 1
        i += 1
    if quote == '"':
        raise ValueError('multi-line string literal')
    return delta


def rust_test_only(old, new):
    """True when only lines inside the trailing test module differ."""
    try:
        old_lines, new_lines = old.decode().split('\n'), new.decode().split('\n')
        old_start, new_start = test_module_start(old_lines), test_module_start(new_lines)
    except (UnicodeDecodeError, ValueError):
        return False
    return old_start is not None and new_start is not None and old_lines[:old_start+2] == new_lines[:new_start+2]


def violation(worker, entry):
    path, status = entry['path'], entry['status']
    if not path.isascii() or not path.isprintable():
        return 'non-ASCII or control character in path'
    if protected(path):
        return 'protected path'
    if status not in 'AMD':
        return f'unsupported change type {status}'
    for mode in (entry['old_mode'], entry['new_mode']):
        if mode != '000000' and mode not in REGULAR_MODES:
            return f'not a regular file (mode {mode})'
    if status == 'M' and entry['old_mode'] != entry['new_mode']:
        return 'file mode change'
    if status == 'A' and entry['new_mode'] != '100644':
        return 'new file is not mode 100644'
    old, new = blob(entry['old']), blob(entry['new'])
    if b'\0' in old or b'\0' in new:
        return 'binary content'
    if worker == 'test-gap':
        if status == 'D':
            return 'test-gap does not delete files'
        if path.startswith(TEST_PREFIXES):
            return None
        if path.startswith(RUST_SOURCE_PREFIX) and path.endswith('.rs') and status == 'M':
            return None if rust_test_only(old, new) else 'change outside the trailing #[cfg(test)] module'
        return 'not a test path'
    if path.startswith(CLEANUP_PREFIXES) or path in CLEANUP_FILES:
        return None
    return 'not a cleanup path'


def check(worker, base, head):
    entries = changes(base, head)
    rejected = [{'path': e['path'], 'reason': reason} for e in entries if (reason := violation(worker, e))]
    return {'status': 'FAIL' if rejected else 'PASS', 'worker': worker,
            'base': git('rev-parse', '--verify', f'{base}^{{commit}}').stdout.decode().strip(),
            'head': git('rev-parse', '--verify', f'{head}^{{commit}}').stdout.decode().strip(),
            'changed': len(entries), 'rejected': rejected,
            'limits': ['paths, modes and test-module boundary only; not whether a change is needed or correct']}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('worker', choices=WORKERS)
    parser.add_argument('--base', default='origin/main')
    parser.add_argument('--head', default='HEAD')
    args = parser.parse_args()
    try:
        result = check(args.worker, args.base, args.head)
    except (RuntimeError, ValueError, IndexError) as exc:
        print(json.dumps({'status': 'UNKNOWN', 'error': str(exc)}, ensure_ascii=False))
        sys.exit(2)
    print(json.dumps(result, ensure_ascii=False, indent=2))
    sys.exit(0 if result['status'] == 'PASS' else 1)


if __name__ == '__main__':
    main()
