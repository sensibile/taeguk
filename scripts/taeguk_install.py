#!/usr/bin/env python3
"""Install Taeguk for coding agents: user-level init skill and CLI, project-level bundles."""
import argparse
from functools import lru_cache
import hashlib
import json
import os
from pathlib import Path
import re
import shlex
import subprocess
import sysconfig
import tomllib

import install_codex as engine

AGENTS = ('codex', 'claude')
MODELS = {'gpt-6-astra': 'opus', 'gpt-6-sol': 'opus', 'gpt-6-luna': 'sonnet'}
EFFORTS = {'low', 'medium', 'high'}
SKILL_REF = re.compile(r'\$(taeguk-[a-z0-9-]+)')
EXE = sysconfig.get_config_var('EXE') or ''
CLI = '.local/bin/taeguk' + EXE
INIT = {'codex': '.agents/skills/taeguk-init/SKILL.md', 'claude': '.claude/skills/taeguk-init/SKILL.md'}
PROJECT_SKILLS = {'codex': '.agents/skills', 'claude': '.claude/skills'}
LEGACY_RECEIPTS = ('.codex/taeguk-install.json',)
# Paths each component may own; a receipt naming anything else is rejected before removal.
OWNED = {'rules': ('.taeguk/rules/',), 'claude': ('.claude/skills/taeguk-*/', '.claude/agents/taeguk-*.md'),
         'codex': ('.agents/skills/taeguk-*/', '.codex/agents/taeguk_*.toml'),
         'init-claude': (INIT['claude'],), 'init-codex': (INIT['codex'],), 'cli': (CLI,)}


GENERATED_DIRS = {'__pycache__'}
GENERATED_FILES = {'.DS_Store'}
GENERATED_SUFFIXES = {'.pyc', '.pyo'}


def git_env():
    """Environment for Git calls on the source checkout, free of a caller's GIT_DIR (e.g. inside hooks)."""
    return {key: value for key, value in os.environ.items() if not key.startswith('GIT_')}


def receipt(component):
    return f'.taeguk/receipts/{component}.json'


@lru_cache(maxsize=None)
def own_repository(source):
    """Whether source is the top level of its own Git checkout, not a directory inside another repository."""
    try:
        top = subprocess.run(['git', '-C', str(source), 'rev-parse', '--show-toplevel'], capture_output=True, text=True,
                             check=True, env=git_env()).stdout.strip()
    except (OSError, subprocess.CalledProcessError):
        return False
    return Path(top).resolve() == Path(source).resolve()


def source_version(source):
    def git(*args):
        return subprocess.run(['git', '-C', str(source), *args], capture_output=True, text=True, check=True, env=git_env()).stdout.strip()
    # An archive unpacked inside another repository would otherwise report that repository's HEAD.
    if not own_repository(source):
        return 'unknown'
    try:
        head = git('rev-parse', 'HEAD')
        dirty = git('status', '--porcelain', '--', 'codex', 'docs/rules', 'installer', 'scripts/taeguk_install.py',
                    'scripts/install_codex.py', 'tools/taeguk-cli')
    except (OSError, subprocess.CalledProcessError):
        return 'unknown'
    return head + ('+dirty' if dirty else '')


def git_ignored(source, paths):
    """Paths Git ignores in the source checkout; none when Git is unavailable."""
    if not paths:
        return set()
    try:
        out = subprocess.run(['git', '-C', str(source), 'check-ignore', '--stdin', '-z'], capture_output=True, env=git_env(),
                             input='\0'.join(str(p.relative_to(source)) for p in paths).encode())
    except OSError:
        return set()
    if out.returncode not in (0, 1):
        return set()
    return {source/name for name in out.stdout.decode().split('\0') if name}


def tracked_files(source, directory, skip=()):
    """Regular files under directory, without Git-ignored ones; directories in skip are never entered."""
    # Compare lexical paths: resolving would let an ignored link stand in for the tracked path it points to.
    lexical = lambda path: Path(os.path.abspath(path))
    if not directory.is_dir():
        return []
    skip = {lexical(path) for path in skip}
    # Symlinks are refused below the walk root too, so the root and its ancestors up to source must be real.
    relative = directory.relative_to(source)
    for depth in range(len(relative.parts) + 1):
        if engine.is_link(source/Path(*relative.parts[:depth])):
            raise ValueError('symlink source is not supported')
    # One Git call lists ignored untracked entries (directories collapsed) under this directory. Only the
    # source's own repository decides what is ignored; a surrounding repository's rules do not apply.
    listed = []
    if own_repository(source):
        # In a real repository a failed listing must stop the install; treating it as "nothing ignored"
        # would bundle ignored caches or secrets.
        try:
            out = subprocess.run(['git', '-C', str(source), 'ls-files', '-z', '--others', '--ignored', '--exclude-standard',
                                  '--directory', '--', str(directory.relative_to(source))],
                                 capture_output=True, env=git_env())
        except OSError as error:
            raise ValueError(f'cannot list Git-ignored source files: {error}')
        if out.returncode != 0:
            raise ValueError('cannot list Git-ignored source files: ' + out.stderr.decode().strip())
        listed = out.stdout.decode().split('\0')
    ignored_entries = {lexical(source/name.rstrip('/')) for name in listed if name}
    def unreadable(error):
        raise ValueError(f'cannot read source directory: {error.filename}')
    files = []
    # onerror: an unreadable directory must fail the walk, not silently drop its files from the bundle.
    for current, dirs, names in os.walk(directory, onerror=unreadable):
        here = lexical(current)
        # Build output (e.g. Cargo's target, even when it links to an external cache) is never entered.
        dirs[:] = [name for name in dirs if here/name not in skip]
        # Ignored local artifacts (caches, links) are neither entered nor validated. Generated files are
        # dropped even without Git (e.g. an unpacked archive after the test suite ran).
        dirs[:] = [name for name in dirs if here/name not in ignored_entries and name not in GENERATED_DIRS]
        names = [name for name in names if here/name not in ignored_entries
                 and name not in GENERATED_FILES and Path(name).suffix not in GENERATED_SUFFIXES]
        for name in dirs + names:
            if engine.is_link(here/name):
                raise ValueError('symlink source is not supported')
        files += [here/name for name in names if (here/name).is_file()]
    return sorted(files)


def lf(rel, data):
    """Install text with LF endings so receipts match on every platform's checkout."""
    return data.replace(b'\r\n', b'\n') if Path(rel).suffix in engine.TEXT_SUFFIXES else data


def roles(source):
    found = []
    for src in tracked_files(source, source/'codex/agents'):
        if src.parent != source/'codex/agents' or not src.match('taeguk_*.toml'):
            continue
        role = tomllib.loads(src.read_text())
        if role['name'] != src.stem or not role.get('description') or not role.get('developer_instructions'):
            raise ValueError(f'invalid agent: {src.name}')
        found.append((src, role))
    if not found:
        raise ValueError('bundle has no agents')
    return found


def skill_files(source):
    skills = sorted((source/'codex/skills').glob('taeguk-*'))
    ignored = git_ignored(source, skills) if own_repository(source) else set()
    skills = [skill for skill in skills if skill not in ignored]
    if not skills:
        raise ValueError('bundle has no skills')
    for skill in skills:
        if engine.is_link(skill) or not (skill/'SKILL.md').is_file():
            raise ValueError('invalid skill directory')
        for src in tracked_files(source, skill):
            yield skill.name, src.relative_to(skill), src.read_bytes()


def claude_agent(role):
    name = role['name'].replace('_', '-')
    lines = ['---', f'name: {name}', f'description: {json.dumps(role["description"])}']
    if 'model' in role:
        if role['model'] not in MODELS:
            raise ValueError(f'unmapped agent model: {role["model"]}')
        lines.append(f'model: {MODELS[role["model"]]}')
    if 'model_reasoning_effort' in role:
        if role['model_reasoning_effort'] not in EFFORTS:
            raise ValueError(f'unsupported reasoning effort: {role["model_reasoning_effort"]}')
        lines.append(f'effort: {role["model_reasoning_effort"]}')
    body = SKILL_REF.sub(r'/\1', role['developer_instructions'].strip())
    return name, ('\n'.join(lines + ['---', '', body]) + '\n').encode()


def project_bundle(agent, source):
    files = {}
    for src, role in roles(source):
        if agent == 'codex':
            files[f'.codex/agents/{src.name}'] = (lf(src.name, src.read_bytes()), 0o644)
        else:
            name, data = claude_agent(role)
            if f'.claude/agents/{name}.md' in files:
                raise ValueError(f'roles collide after converting _ to - for Claude: {name}')
            files[f'.claude/agents/{name}.md'] = (data, 0o644)
    for skill, rel, data in skill_files(source):
        if agent == 'claude':
            if rel.parts[0] == 'agents':
                continue
            if rel.suffix == '.md':
                data = SKILL_REF.sub(r'/\1', data.decode()).encode()
        files[f'{PROJECT_SKILLS[agent]}/{skill}/{rel.as_posix()}'] = (lf(rel, data), 0o644)
    return files


REPO_BLOB = b'https://github.com/sensibile/taeguk/blob/'


def published_commit(source):
    """Newest commit of a clean source that is also on origin/main.

    Rule links pin to it rather than to HEAD: a squash-merged feature branch is deleted, but main keeps
    its commits, so the pinned links keep resolving.
    """
    if not re.fullmatch(r'[0-9a-f]{40}', source_version(source)):
        return None
    try:
        out = subprocess.run(['git', '-C', str(source), 'merge-base', 'HEAD', 'origin/main'], capture_output=True,
                             text=True, check=True, env=git_env()).stdout.strip()
    except (OSError, subprocess.CalledProcessError):
        return None
    return out if re.fullmatch(r'[0-9a-f]{40}', out) else None


def rules_bundle(source, link_pin=None):
    """Rule snapshot; link_pin overrides the computed commit ('' leaves links on main)."""
    rules = source/'docs/rules'
    # Links into this repository follow a commit on main, so a snapshot never cites a later main.
    commit = published_commit(source) if link_pin is None else link_pin
    pin = commit.encode() if commit else None
    files = {}
    for src in tracked_files(source, rules):
        data = lf(src.name, src.read_bytes())
        if pin and src.suffix == '.md':
            data = data.replace(REPO_BLOB + b'main/', REPO_BLOB + pin + b'/')
        files[f'.taeguk/rules/{src.relative_to(rules).as_posix()}'] = (data, 0o644)
    if '.taeguk/rules/registry.toml' not in files:
        raise ValueError('bundle has no rule registry')
    return files


def init_bundle(agent, source):
    if any(char in str(source) for char in '`\r\n'):
        raise ValueError('the Taeguk source path must not contain backticks or line breaks')
    template = (source/'installer/taeguk-init.md').read_text()
    front = ['---', 'name: taeguk-init',
             'description: "Install or update the Taeguk project bundle in the current project; use only when the user explicitly asks to initialize Taeguk."']
    if agent == 'claude':
        front.append('disable-model-invocation: true')
    installer = str(source/'scripts/taeguk_install.py')
    if os.name == 'nt':
        # PowerShell single-quoted string: only ' needs escaping, by doubling it.
        installer, quoting = "'" + installer.replace("'", "''") + "'", "a PowerShell single-quoted argument (double any `'` in it)"
    else:
        installer, quoting = shlex.quote(installer), "a POSIX single-quoted argument (write any `'` in it as `'\\''`)"
    values = {'INSTALLER': installer, 'QUOTING': quoting, 'SOURCE': str(source), 'AGENT': agent}
    # One pass, so a value containing a token (e.g. a path with "{{AGENT}}") is never substituted again.
    body = re.sub(r'\{\{(INSTALLER|QUOTING|SOURCE|AGENT)\}\}', lambda match: values[match.group(1)], template)
    return {INIT[agent]: (('\n'.join(front + ['---', '']) + body).encode(), 0o644)}


def cargo_target_dir(source):
    """Cargo's own target directory, which CARGO_TARGET_DIR or build.target-dir may move."""
    crate = source/'tools/taeguk-cli'
    env = dict(os.environ)
    if env.get('CARGO_TARGET_DIR'):
        # Cargo resolves a relative CARGO_TARGET_DIR against the build's working directory; the caller's cwd
        # stands in for it, since metadata itself runs from the crate.
        env['CARGO_TARGET_DIR'] = os.path.abspath(env['CARGO_TARGET_DIR'])
    try:
        out = subprocess.run(['cargo', 'metadata', '--format-version', '1', '--no-deps', '--manifest-path',
                              str(crate/'Cargo.toml')], capture_output=True, text=True, check=True, cwd=crate, env=env)
        return Path(json.loads(out.stdout)['target_directory'])
    except (OSError, subprocess.CalledProcessError, ValueError, KeyError):
        return Path(env['CARGO_TARGET_DIR']) if env.get('CARGO_TARGET_DIR') else crate/'target'


def release_binary(target):
    """The release CLI under Cargo's target dir, including a build.target triple subdirectory."""
    name = 'taeguk' + EXE
    triple = os.environ.get('CARGO_BUILD_TARGET')
    if triple:
        return target/triple/'release'/name
    found = [path for path in [target/'release'/name, *sorted(target.glob(f'*/release/{name}'))]
             if path.is_file() and not path.is_symlink()]
    if len(found) > 1:
        raise ValueError('several release CLIs found; set CARGO_BUILD_TARGET to choose one: '
                         + ', '.join(str(path) for path in found))
    return found[0] if found else target/'release'/name


def cli_bundle(source):
    target = cargo_target_dir(source)
    binary = release_binary(target)
    if not binary.is_file() or binary.is_symlink():
        raise ValueError('build the release CLI before installation')
    # A checkout that changed CLI sources after the last build refreshes their mtimes past the binary.
    crate = source/'tools/taeguk-cli'
    inputs = tracked_files(source, crate, skip=(crate/'target', target))
    if inputs and binary.stat().st_mtime < max(src.stat().st_mtime for src in inputs):
        raise ValueError('the release CLI is older than its sources; rebuild it before installation')
    return {CLI: (binary.read_bytes(), 0o755)}


def verify_receipt(root, name):
    path = engine.safe_target(root, receipt(name))
    if not path.exists():
        raise ValueError(f'no managed installation: {name}')
    doc = json.loads(path.read_text())
    if not isinstance(doc, dict) or doc.get('schema_version') != 1 or not isinstance(doc.get('files'), dict) or not doc['files']:
        raise ValueError(f'invalid installation receipt: {name}')
    for key, expected in doc['files'].items():
        if not engine.owns(OWNED[name], key):
            raise ValueError(f'receipt {name} claims a path outside its component: {key}')
        target = engine.safe_target(root, key)
        if not target.exists() or not engine.unchanged(target.read_bytes(), expected, key):
            raise ValueError(f'managed file is missing or locally modified: {key}')


def installed_versions(root, names):
    """Source version each component's receipt recorded, read before this command changes anything."""
    versions = {}
    for name in names:
        path = root/receipt(name)
        if path.is_file():
            doc = json.loads(path.read_text())
            versions[name] = doc.get('source_version', 'unknown') if isinstance(doc, dict) else 'unknown'
    return versions


def run(components, source, root, mode, meta):
    """Preflight every component before changing any, then run them in order."""
    for name, build in components:
        if mode == 'apply':
            engine.execute(source, root, 'dry-run', build, receipt(name), meta, OWNED[name])
        elif mode == 'uninstall':
            verify_receipt(root, name)
    return {name: engine.execute(source, root, mode, build, receipt(name), meta, OWNED[name]) for name, build in components}


def installed(root, prefix=''):
    return [agent for agent in AGENTS if (root/receipt(prefix+agent)).exists()]


def remaining(root, agents, prefix=''):
    """Agents installed at root that this command does not touch."""
    return [agent for agent in installed(root, prefix) if agent not in agents]


def user(source, home, agents, mode, with_cli=True):
    components = [(f'init-{agent}', lambda s, a=agent: init_bundle(a, s)) for agent in agents]
    kept = remaining(home, agents, 'init-') if mode == 'uninstall' else []
    cli_managed = (home/receipt('cli')).exists()
    if with_cli and not kept and (mode in ('dry-run', 'apply') or cli_managed):
        components.append(('cli', cli_bundle))
    version = source_version(source)
    before = installed_versions(home, [name for name, _ in components])
    result = {'scope': 'user', 'root': str(home), 'installer_version': version, 'installed_versions': before,
              'components': run(components, source, home, mode, {'source_root': str(source), 'source_version': version})}
    if with_cli and kept:
        result['kept'] = {'cli': kept}
    elif with_cli and mode in ('check', 'uninstall') and not cli_managed:
        result['cli'] = 'not installed'
    elif not with_cli and mode == 'check':
        if cli_managed:
            verify_receipt(home, 'cli')
            if os.name != 'nt' and not (home/CLI).stat().st_mode & 0o111:
                raise ValueError('installed CLI is not executable')
            result['cli'] = 'matches its receipt (not compared with a fresh build)'
        else:
            result['cli'] = 'not installed'
    legacy = [path for path in LEGACY_RECEIPTS if (home/path).exists()]
    if legacy:
        result['legacy'] = {'receipts': legacy,
                            'action': 'run `taeguk_install.py migrate`, then `install_codex.py --uninstall` to remove global roles and skills'}
    return result


def parse_agents(text):
    agents = [agent for agent in text.split(',') if agent]
    if not agents or set(agents) - set(AGENTS) or len(set(agents)) != len(agents):
        raise ValueError('agents must be distinct values from: '+', '.join(AGENTS))
    return agents


def select_agents(text, mode, root, prefix=''):
    """Explicit agents, else every agent for install and the installed ones for check/uninstall."""
    if text:
        return parse_agents(text)
    if mode in ('check', 'uninstall'):
        agents = installed(root, prefix)
        # A scope holding only its shared component (user CLI after migrate, project rules after a
        # partial install) is still a managed installation.
        shared = 'cli' if prefix == 'init-' else 'rules'
        if not agents and not (root/receipt(shared)).exists():
            raise ValueError('no managed installation')
        return agents
    return list(AGENTS)


def project(source, root, home, agents, mode):
    if not root.is_dir():
        raise ValueError(f'project root is not an existing directory: {root}')
    kept = remaining(root, agents) if mode == 'uninstall' else []
    # Updating the shared rules alone would leave other installed agents on an older version.
    synced = remaining(root, agents) if mode in ('dry-run', 'apply', 'check') else []
    agents = list(agents) + synced
    # The link pin depends on the checkout's remote refs, so --check reuses the pin the install recorded
    # and the same source commit always rebuilds the same rule bytes.
    stored = None
    rules_receipt = root/receipt('rules')
    if mode == 'check' and rules_receipt.is_file():
        doc = json.loads(rules_receipt.read_text())
        stored = doc.get('link_pin') if isinstance(doc, dict) else None
    link_pin = stored if isinstance(stored, str) else (published_commit(source) or '')
    components = [] if kept else [('rules', lambda s: rules_bundle(s, link_pin))]
    components += [(agent, lambda s, a=agent: project_bundle(a, s)) for agent in agents]
    version = source_version(source)
    before = installed_versions(root, [name for name, _ in components])
    result = {'scope': 'project', 'root': str(root), 'installer_version': version, 'installed_versions': before,
              'components': run(components, source, root, mode, {'source_version': version, 'link_pin': link_pin})}
    if kept:
        result['kept'] = {'rules': kept}
    if synced:
        result['synced_agents'] = synced
    if mode != 'uninstall':
        managed = [receipt(name) for name, _ in components] + [key for _, build in components for key in build(source)]
        ignored = sorted(path.relative_to(root).as_posix() for path in git_ignored(root, [root/key for key in managed]))
        if ignored:
            result['ignored_by_git'] = {'count': len(ignored), 'examples': ignored[:5],
                                        'action': 'add .gitignore exceptions so the installed files and receipts can be committed'}
        names = sorted({skill for skill, _, _ in skill_files(source)})
        user_skills = {'claude': '.claude/skills', 'codex': '.agents/skills'}
        shadowed = {agent: found for agent in agents
                    if (found := [name for name in names if (home/user_skills[agent]/name).exists()])}
        if shadowed:
            result['shadowed_by_user_skills'] = shadowed
    return result


def migrate(home, mode):
    legacy_path = engine.safe_target(home, LEGACY_RECEIPTS[0])
    target = engine.safe_target(home, receipt('cli'))
    if not legacy_path.exists():
        raise ValueError('no legacy Codex receipt')
    legacy = json.loads(legacy_path.read_text())
    if not isinstance(legacy, dict) or legacy.get('schema_version') != 1 or not isinstance(legacy.get('files'), dict):
        raise ValueError('invalid installation receipt')
    outside = [key for key in legacy['files'] if not engine.owns(engine.LEGACY_OWNED, key)]
    if outside:
        raise ValueError(f'receipt claims paths outside its component: {outside[0]}')
    expected = legacy['files'].get(CLI)
    if expected is None:
        return {'status': 'nothing to migrate'}
    if target.exists():
        raise ValueError('CLI receipt already exists')
    cli = engine.safe_target(home, CLI)
    if not cli.exists() or hashlib.sha256(cli.read_bytes()).hexdigest() != expected:
        raise ValueError(f'managed file is missing or locally modified: {CLI}')
    plan = {'move': CLI, 'from': LEGACY_RECEIPTS[0], 'to': receipt('cli')}
    if mode == 'dry-run':
        return {'status': 'planned', **plan}
    remaining = {key: value for key, value in legacy['files'].items() if key != CLI}
    new = {'schema_version': 1, 'source_root': legacy.get('source_root'), 'files': {CLI: expected}}
    engine.atomic_write(target, json.dumps(new, indent=2).encode()+b'\n', 0o600)
    try:
        engine.atomic_write(legacy_path, json.dumps({**legacy, 'files': remaining}, indent=2).encode()+b'\n', 0o600)
    except Exception:
        target.unlink()
        raise
    return {'status': 'migrated', **plan}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--home', type=Path, default=Path.home(), help='user home; temporary path for tests')
    commands = parser.add_subparsers(dest='command', required=True)
    user_cmd = commands.add_parser('user', help='install taeguk-init and the CLI at user level')
    user_cmd.add_argument('--no-cli', action='store_true')
    project_cmd = commands.add_parser('project', help='install the Taeguk bundle into a project')
    project_cmd.add_argument('--root', type=Path, required=True)
    for command in (user_cmd, project_cmd):
        command.add_argument('--agent', help='comma-separated: '+', '.join(AGENTS)+'; default all for install, installed ones for check/uninstall')
        group = command.add_mutually_exclusive_group()
        for name in ('apply', 'check', 'uninstall'):
            group.add_argument('--'+name, action='store_true')
    migrate_cmd = commands.add_parser('migrate', help='move the CLI out of the legacy Codex receipt')
    migrate_cmd.add_argument('--apply', action='store_true')
    args = parser.parse_args()
    source = Path(__file__).resolve().parents[1]
    home = args.home.resolve()
    mode = next((name for name in ('apply', 'check', 'uninstall') if getattr(args, name, False)), 'dry-run')
    try:
        if args.command == 'migrate':
            result = migrate(home, mode)
        else:
            root = home if args.command == 'user' else args.root.resolve()
            agents = select_agents(args.agent, mode, root, 'init-' if args.command == 'user' else '')
            if args.command == 'user':
                result = user(source, home, agents, mode, not args.no_cli)
            else:
                result = project(source, args.root.resolve(), home, agents, mode)
    except (OSError, ValueError, KeyError) as exc:
        parser.exit(2, json.dumps({'error': str(exc)}, ensure_ascii=False)+'\n')
    print(json.dumps(result, ensure_ascii=False, indent=2))


if __name__ == '__main__':
    main()
