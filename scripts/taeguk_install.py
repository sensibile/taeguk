#!/usr/bin/env python3
"""Install Taeguk for coding agents: user-level init skill and CLI, project-level bundles."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import tomllib

import install_codex as engine

AGENTS = ('codex', 'claude')
MODELS = {'gpt-6-astra': 'opus', 'gpt-6-luna': 'sonnet'}
# Mapping awaits BOSS confirmation; these roles inherit the session model until then.
PENDING_MODELS = {'gpt-6-sol'}
EFFORTS = {'low', 'medium', 'high'}
SKILL_REF = re.compile(r'\$(taeguk-[a-z0-9-]+)')
CLI = '.local/bin/taeguk'
INIT = {'codex': '.agents/skills/taeguk-init/SKILL.md', 'claude': '.claude/skills/taeguk-init/SKILL.md'}
PROJECT_SKILLS = {'codex': '.agents/skills', 'claude': '.claude/skills'}
LEGACY_RECEIPTS = ('.codex/taeguk-install.json',)


def receipt(component):
    return f'.taeguk/receipts/{component}.json'


def source_version(source):
    def git(*args):
        return subprocess.run(['git', '-C', str(source), *args], capture_output=True, text=True, check=True).stdout.strip()
    try:
        head = git('rev-parse', 'HEAD')
        dirty = git('status', '--porcelain', '--', 'codex', 'docs/rules', 'installer', 'scripts/taeguk_install.py',
                    'scripts/install_codex.py', 'tools/taeguk-cli')
    except (OSError, subprocess.CalledProcessError):
        return 'unknown'
    return head + ('+dirty' if dirty else '')


def roles(source):
    found = []
    for src in sorted((source/'codex/agents').glob('taeguk_*.toml')):
        if src.is_symlink():
            raise ValueError('symlink source is not supported')
        role = tomllib.loads(src.read_text())
        if role['name'] != src.stem or not role.get('description') or not role.get('developer_instructions'):
            raise ValueError(f'invalid agent: {src.name}')
        found.append((src, role))
    if not found:
        raise ValueError('bundle has no agents')
    return found


def skill_files(source):
    skills = sorted((source/'codex/skills').glob('taeguk-*'))
    if not skills:
        raise ValueError('bundle has no skills')
    for skill in skills:
        if skill.is_symlink() or not (skill/'SKILL.md').is_file():
            raise ValueError('invalid skill directory')
        for src in sorted(skill.rglob('*')):
            if src.is_symlink():
                raise ValueError('symlink source is not supported')
            if src.is_file():
                yield skill.name, src.relative_to(skill), src.read_bytes()


def claude_agent(role):
    name = role['name'].replace('_', '-')
    lines = ['---', f'name: {name}', f'description: {json.dumps(role["description"])}']
    if 'model' in role and role['model'] not in PENDING_MODELS:
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
            files[f'.codex/agents/{src.name}'] = (src.read_bytes(), 0o644)
        else:
            name, data = claude_agent(role)
            files[f'.claude/agents/{name}.md'] = (data, 0o644)
    for skill, rel, data in skill_files(source):
        if agent == 'claude':
            if rel.parts[0] == 'agents':
                continue
            if rel.suffix == '.md':
                data = SKILL_REF.sub(r'/\1', data.decode()).encode()
        files[f'{PROJECT_SKILLS[agent]}/{skill}/{rel.as_posix()}'] = (data, 0o644)
    return files


def rules_bundle(source):
    rules = source/'docs/rules'
    files = {}
    for src in sorted(rules.rglob('*')):
        if src.is_symlink():
            raise ValueError('symlink source is not supported')
        if src.is_file():
            files[f'.taeguk/rules/{src.relative_to(rules).as_posix()}'] = (src.read_bytes(), 0o644)
    if '.taeguk/rules/registry.toml' not in files:
        raise ValueError('bundle has no rule registry')
    return files


def init_bundle(agent, source):
    template = (source/'installer/taeguk-init.md').read_text()
    front = ['---', 'name: taeguk-init',
             'description: "Install or update the Taeguk project bundle in the current project; use only when the user explicitly asks to initialize Taeguk."']
    if agent == 'claude':
        front.append('disable-model-invocation: true')
    body = template.replace('{{SOURCE}}', str(source)).replace('{{AGENT}}', agent)
    return {INIT[agent]: (('\n'.join(front + ['---', '']) + body).encode(), 0o644)}


def cli_bundle(source):
    binary = source/'tools/taeguk-cli/target/release/taeguk'
    if not binary.is_file() or binary.is_symlink():
        raise ValueError('build the release CLI before installation')
    return {CLI: (binary.read_bytes(), 0o755)}


def verify_receipt(root, name):
    path = engine.safe_target(root, receipt(name))
    if not path.exists():
        raise ValueError(f'no managed installation: {name}')
    doc = json.loads(path.read_text())
    if not isinstance(doc, dict) or doc.get('schema_version') != 1 or not isinstance(doc.get('files'), dict):
        raise ValueError('invalid installation receipt')
    for key, expected in doc['files'].items():
        target = engine.safe_target(root, key)
        if not target.exists() or engine.digest(target.read_bytes()) != expected:
            raise ValueError(f'managed file is missing or locally modified: {key}')


def run(components, source, root, mode, meta):
    """Preflight every component before changing any, then run them in order."""
    for name, build in components:
        if mode == 'apply':
            engine.execute(source, root, 'dry-run', build, receipt(name), meta)
        elif mode == 'uninstall':
            verify_receipt(root, name)
    return {name: engine.execute(source, root, mode, build, receipt(name), meta) for name, build in components}


def remaining(root, agents, prefix=''):
    """Agents installed at root that this command does not touch."""
    return [agent for agent in AGENTS if agent not in agents and (root/receipt(prefix+agent)).exists()]


def user(source, home, agents, mode, with_cli=True):
    components = [(f'init-{agent}', lambda s, a=agent: init_bundle(a, s)) for agent in agents]
    kept = remaining(home, agents, 'init-') if mode == 'uninstall' else []
    if with_cli and not kept:
        components.append(('cli', cli_bundle))
    result = {'scope': 'user', 'root': str(home), 'components': run(components, source, home, mode,
              {'source_root': str(source), 'source_version': source_version(source)})}
    if with_cli and kept:
        result['kept'] = {'cli': kept}
    legacy = [path for path in LEGACY_RECEIPTS if (home/path).exists()]
    if legacy:
        result['legacy'] = {'receipts': legacy,
                            'action': 'run `taeguk_install.py migrate`, then `install_codex.py --uninstall` to remove global roles and skills'}
    return result


def project(source, root, home, agents, mode):
    kept = remaining(root, agents) if mode == 'uninstall' else []
    components = [] if kept else [('rules', rules_bundle)]
    components += [(agent, lambda s, a=agent: project_bundle(a, s)) for agent in agents]
    version = source_version(source)
    result = {'scope': 'project', 'root': str(root), 'source_version': version,
              'components': run(components, source, root, mode, {'source_version': version})}
    if kept:
        result['kept'] = {'rules': kept}
    if 'claude' in agents:
        pending = sorted(f'{role["name"]}={role["model"]}' for _, role in roles(source) if role.get('model') in PENDING_MODELS)
        if pending:
            result['unmapped_models'] = pending
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
        command.add_argument('--agent', default=','.join(AGENTS), help='comma-separated: '+', '.join(AGENTS))
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
            agents = [agent for agent in args.agent.split(',') if agent]
            if not agents or set(agents) - set(AGENTS):
                raise ValueError('unsupported agent: '+args.agent)
            if args.command == 'user':
                result = user(source, home, agents, mode, not args.no_cli)
            else:
                result = project(source, args.root.resolve(), home, agents, mode)
    except (OSError, ValueError, KeyError) as exc:
        parser.exit(2, json.dumps({'error': str(exc)}, ensure_ascii=False)+'\n')
    print(json.dumps(result, ensure_ascii=False, indent=2))


if __name__ == '__main__':
    main()
