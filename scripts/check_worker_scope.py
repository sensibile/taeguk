#!/usr/bin/env python3
"""Check that a scheduled worker PR changes only what its worker may change, before auto-merge.

This checks paths, file modes and, for Rust sources, that test-gap edits stay inside the trailing
`#[cfg(test)]` module. It does not judge whether a change is needed or correct.
"""
import argparse
import json
import re
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

# CI runs `python3 scripts/...` and `unittest discover -s tests`, which put these directories first
# on sys.path: a module there named like a standard one replaces it in protected checks.
IMPORT_ROOTS = ('scripts/', 'tests/')
SHADOWED = set(sys.stdlib_module_names) | {'sitecustomize', 'usercustomize'}

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


def significant(src):
    """Positions of code characters outside comments and literals; a literal counts as one `L`.

    Handles line and nested block comments, strings with escapes over several lines, raw and
    byte strings, char literals and lifetimes. An unterminated comment or literal raises.
    """
    out, i, n = [], 0, len(src)
    while i < n:
        c = src[i]
        if c.isspace():
            i += 1
        elif src.startswith('//', i):
            j = src.find('\n', i)
            i = n if j < 0 else j
        elif src.startswith('/*', i):
            depth, i = 1, i + 2
            while depth:
                if i >= n:
                    raise ValueError('unterminated block comment')
                if src.startswith('/*', i):
                    depth, i = depth + 1, i + 2
                elif src.startswith('*/', i):
                    depth, i = depth - 1, i + 2
                else:
                    i += 1
        elif raw := re.match(r'[bc]?r(#*)"', src[i:]) if (c in 'bcr' and (i == 0 or not (src[i-1].isalnum() or src[i-1] == '_'))) else None:
            close = '"' + raw.group(1)
            j = src.find(close, i + raw.end())
            if j < 0:
                raise ValueError('unterminated raw string')
            out.append((i, 'L'))
            i = j + len(close)
        elif c == '"':
            j = i + 1
            while j < n and src[j] != '"':
                j += 2 if src[j] == '\\' else 1
            if j >= n:
                raise ValueError('unterminated string')
            out.append((i, 'L'))
            i = j + 1
        elif c == "'":
            if src.startswith('\\', i + 1):
                j = src.find("'", i + 3)
                if j < 0:
                    raise ValueError('unterminated char literal')
                out.append((i, 'L'))
                i = j + 1
            elif i + 2 < n and src[i+2] == "'":
                out.append((i, 'L'))
                i += 3
            else:
                out.append((i, c))  # lifetime marker
                i += 1
        else:
            out.append((i, c))
            i += 1
    return out


def test_module_open(src):
    """Offset of the `{` opening the trailing `#[cfg(test)] mod name {`, or None.

    The module must be the only `#[cfg(test)]` item and the last item: nothing but whitespace and
    comments may follow its closing brace.
    """
    tokens = significant(src)
    text = ''.join(c for _, c in tokens)
    attr = '#[cfg(test)]'
    if text.count(attr) != 1:
        return None
    k = text.index(attr) + len(attr)
    after_attr = tokens[k-1][0] + 1
    opening = next((j for j in range(k, len(tokens)) if tokens[j][1] in '{;'), None)
    if opening is None or tokens[opening][1] != '{' or not re.fullmatch(r'\s*mod\s+[A-Za-z_]\w*\s*', src[after_attr:tokens[opening][0]]):
        return None
    depth = 0
    for j in range(opening, len(tokens)):
        depth += {'{': 1, '}': -1}.get(tokens[j][1], 0)
        if depth == 0:
            return tokens[opening][0] if j == len(tokens) - 1 else None
    return None


def rust_test_only(old, new):
    """True when old and new are identical up to the trailing test module's opening brace."""
    try:
        old_src, new_src = old.decode(), new.decode()
        old_open, new_open = test_module_open(old_src), test_module_open(new_src)
    except (UnicodeDecodeError, ValueError):
        return False
    return old_open is not None and new_open is not None and old_src[:old_open+1] == new_src[:new_open+1]


def violation(worker, entry):
    path, status = entry['path'], entry['status']
    if not path.isascii() or not path.isprintable():
        return 'non-ASCII or control character in path'
    if protected(path):
        return 'protected path'
    if path.startswith(IMPORT_ROOTS) and status != 'D':
        rest = path.split('/', 1)[1]
        top = rest.split('/')[0]
        if (top.split('.')[0] if top == rest else top).lower() in SHADOWED:
            return 'shadows a standard library module'
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
