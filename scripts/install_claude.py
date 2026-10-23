#!/usr/bin/env python3
"""Install the local Taeguk bundle for Claude Code without modifying unrelated Claude settings."""
import argparse
import json
from pathlib import Path
import re
import tomllib

import install_codex

RECEIPT = '.claude/taeguk-install.json'
CLI = '.local/bin/taeguk'
MODELS = {'gpt-6-astra': 'opus', 'gpt-6-sol': 'opus', 'gpt-6-luna': 'sonnet'}
EFFORTS = {'low', 'medium', 'high'}
SKILL_REF = re.compile(r'\$(taeguk-[a-z0-9-]+)')


def skill_text(data):
    return SKILL_REF.sub(r'/\1', data.decode()).encode()


def agent_markdown(role):
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


def payload(source):
    files = {}
    for src in sorted((source/'codex/agents').glob('taeguk_*.toml')):
        if src.is_symlink():
            raise ValueError('symlink source is not supported')
        role = tomllib.loads(src.read_text())
        if role['name'] != src.stem or not role.get('description') or not role.get('developer_instructions'):
            raise ValueError(f'invalid agent: {src.name}')
        name, data = agent_markdown(role)
        files[f'.claude/agents/{name}.md'] = (data, 0o644)
    if not files:
        raise ValueError('bundle has no agents')
    skills = sorted((source/'codex/skills').glob('taeguk-*'))
    if not skills:
        raise ValueError('bundle has no skills')
    for skill in skills:
        if skill.is_symlink() or not (skill/'SKILL.md').is_file():
            raise ValueError('invalid skill directory')
        for src in sorted(skill.rglob('*')):
            if src.is_symlink():
                raise ValueError('symlink source is not supported')
            rel = src.relative_to(skill)
            if not src.is_file() or rel.parts[0] == 'agents':
                continue
            data = src.read_bytes()
            if src.suffix == '.md':
                data = skill_text(data)
            files[f'.claude/skills/{skill.name}/{rel.as_posix()}'] = (data, 0o644)
    return files


def execute(source, home, mode='dry-run'):
    result = install_codex.execute(source, home, mode, build=payload, receipt_path=RECEIPT)
    result['cli'] = 'present' if (Path(home)/CLI).is_file() else 'missing'
    return result


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--home',type=Path,default=Path.home(),help='installation home; temporary path for tests')
    group=parser.add_mutually_exclusive_group()
    for name in ('apply','check','uninstall'):
        group.add_argument('--'+name,action='store_true')
    args=parser.parse_args()
    mode=next((name for name in ('apply','check','uninstall') if getattr(args,name)),'dry-run')
    try:
        result=execute(Path(__file__).resolve().parents[1],args.home,mode)
    except (OSError,ValueError,KeyError) as exc:
        parser.exit(2,json.dumps({'error':str(exc)},ensure_ascii=False)+'\n')
    print(json.dumps(result,ensure_ascii=False,indent=2))


if __name__=='__main__':
    main()
