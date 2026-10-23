#!/usr/bin/env python3
"""Install the local Taeguk bundle without modifying unrelated Codex settings."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import stat
import tempfile
import tomllib

RECEIPT = '.codex/taeguk-install.json'


def digest(data):
    return hashlib.sha256(data).hexdigest()


def payload(source):
    files = {}
    for src in sorted((source/'codex/agents').glob('taeguk_*.toml')):
        if src.is_symlink():
            raise ValueError('symlink source is not supported')
        data = src.read_bytes()
        role = tomllib.loads(data.decode())
        if role['name'] != src.stem or not role.get('description') or not role.get('developer_instructions'):
            raise ValueError(f'invalid agent: {src.name}')
        files[f'.codex/agents/{src.name}'] = (data, 0o644)
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
            if src.is_file():
                files[f'.agents/skills/{skill.name}/{src.relative_to(skill).as_posix()}'] = (src.read_bytes(), 0o644)
    binary = source/'tools/taeguk-cli/target/release/taeguk'
    if not binary.is_file() or binary.is_symlink():
        raise ValueError('build the release CLI before installation')
    files['.local/bin/taeguk'] = (binary.read_bytes(), 0o755)
    return files


def safe_target(home, relative):
    rel = Path(relative)
    if rel.is_absolute() or any(p in ('..', '.') for p in rel.parts):
        raise ValueError('unsafe installation path')
    path = home
    for part in rel.parts:
        path = path/part
        if path.is_symlink():
            raise ValueError(f'symlink target is not supported: {relative}')
    if path.exists() and not path.is_file():
        raise ValueError(f'target is not a file: {relative}')
    return path


def atomic_write(path, data, mode):
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, name = tempfile.mkstemp(prefix='.taeguk-', dir=path.parent)
    try:
        with os.fdopen(fd, 'wb') as stream:
            stream.write(data)
            stream.flush()
            os.fsync(stream.fileno())
        os.chmod(name, mode)
        os.replace(name, path)
    finally:
        if os.path.exists(name):
            os.unlink(name)


def prune(home, directory):
    """Remove empty directories that belong to Taeguk; shared parents such as ~/.local/bin stay."""
    def owned(path):
        return any(part.startswith(('taeguk', '.taeguk')) for part in path.relative_to(home).parts)
    while directory != home and owned(directory) and directory.is_dir() and not any(directory.iterdir()):
        directory.rmdir()
        directory = directory.parent


def execute(source, home, mode='dry-run', build=None, receipt_path=RECEIPT, meta=None):
    if mode not in ('dry-run', 'apply', 'check', 'uninstall'):
        raise ValueError('unsupported mode')
    source, home = source.resolve(), home.resolve()
    bundle = (build or payload)(source)
    hashes = {key:digest(data) for key,(data,_) in bundle.items()}
    receipt = safe_target(home, receipt_path)
    previous = {}
    if receipt.exists():
        doc = json.loads(receipt.read_text())
        if not isinstance(doc, dict) or doc.get('schema_version') != 1 or not isinstance(doc.get('files'),dict):
            raise ValueError('invalid installation receipt')
        previous = doc['files']
        if not set(previous).issubset(bundle):
            raise ValueError('receipt contains files outside this bundle; use its original installer')
    for key in bundle:
        target = safe_target(home,key)
        if key in previous:
            if not isinstance(previous[key],str) or len(previous[key]) != 64:
                raise ValueError('invalid receipt hash')
            if not target.exists() or digest(target.read_bytes()) != previous[key]:
                raise ValueError(f'managed file is missing or locally modified: {key}')
        elif target.exists():
            raise ValueError(f'unmanaged installation collision: {key}')
    if mode == 'check':
        if previous != hashes:
            raise ValueError('installed bundle does not match current source')
        for key,(_,expected_mode) in bundle.items():
            if expected_mode & 0o111 and not stat.S_IMODE((home/key).stat().st_mode)&0o111:
                raise ValueError('installed CLI is not executable')
        return {'status':'verified','files':len(hashes)}
    if mode == 'dry-run':
        return {'status':'planned','files':sorted(hashes),'home':str(home)}
    if mode == 'uninstall' and not previous:
        raise ValueError('no managed installation')
    targets = list(bundle) if mode == 'apply' else list(previous)
    snapshot, touched = {}, []
    for key in targets+[receipt_path]:
        target = safe_target(home,key)
        snapshot[key] = (target.read_bytes(),stat.S_IMODE(target.stat().st_mode)) if target.exists() else None
    try:
        for key in targets:
            touched.append(key)
            if mode == 'apply':
                atomic_write(home/key,*bundle[key])
            else:
                (home/key).unlink()
                prune(home, (home/key).parent)
        touched.append(receipt_path)
        if mode == 'apply':
            data=json.dumps({'schema_version':1,**({'source_root':str(source)} if meta is None else meta),'files':hashes},indent=2).encode()+b'\n'
            atomic_write(receipt,data,0o600)
        else:
            receipt.unlink()
            prune(home, receipt.parent)
    except Exception:
        for key in reversed(touched):
            saved=snapshot[key]
            if saved is None:
                (home/key).unlink(missing_ok=True)
            else:
                atomic_write(home/key,*saved)
        raise
    return {'status':'installed' if mode=='apply' else 'removed','files':len(targets)}


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
