#!/usr/bin/env python3
"""Validate rule registry and document references; not semantic approval."""
import argparse
import json
from pathlib import Path
import re
import tomllib
from urllib.parse import unquote, urlsplit


def reference(base, value, boundary, markdown_only=False):
    if not isinstance(value, str) or not value:
        raise ValueError('empty or invalid reference')
    parts = urlsplit(value)
    if parts.scheme or parts.netloc or parts.query:
        raise ValueError('reference must be local without query parameters')
    rel = Path(unquote(parts.path))
    if rel.is_absolute():
        raise ValueError('absolute reference')
    target = (base/rel).resolve()
    if not target.is_relative_to(boundary.resolve()) or not target.is_file():
        raise ValueError(f'missing or out-of-root reference: {value}')
    if markdown_only and target.suffix != '.md':
        raise ValueError('rule and approval reference must be Markdown')
    if parts.fragment and unquote(parts.fragment) not in anchors(target.read_text(encoding='utf-8')):
        raise ValueError(f'missing heading anchor: {value}')
    return target


def anchors(text):
    found = {re.sub(r'[^\w\- ]', '', h.lower()).replace(' ', '-')
             for h in re.findall(r'^#{1,6} +(.+)$', text, re.MULTILINE)}
    found.update(re.findall(r'<a\s+id="([^"]+)"', text))
    return found


def check(root):
    root = Path(root).resolve()
    directory = root/'docs/rules'
    errors, seen = [], set()
    try:
        raw = tomllib.loads((directory/'registry.toml').read_text(encoding='utf-8'))
        if set(raw) != {'schema_version', 'rules'} or type(raw.get('schema_version')) is not int or raw['schema_version'] != 1:
            raise ValueError('unsupported registry schema')
        rules = raw['rules']
        if not isinstance(rules, list) or not rules:
            raise ValueError('registry must contain rules')
    except (OSError, UnicodeError, ValueError, KeyError) as exc:
        return [f'registry: {exc}']
    for index, rule in enumerate(rules):
        try:
            if not isinstance(rule, dict) or set(rule)-{'id','version','status','scope','document','enforcement','approval'}:
                raise ValueError('unknown rule field')
            for key in ('id','version','status','scope','document','enforcement'):
                if not isinstance(rule.get(key), str) or not rule[key].strip():
                    raise ValueError(f'missing or invalid {key}')
            ident = rule['id']
            if not re.fullmatch(r'[A-Z]{3}-[0-9]{3}', ident) or ident in seen:
                raise ValueError(f'invalid or duplicate id: {ident}')
            seen.add(ident)
            if not re.fullmatch(r'[0-9]+\.[0-9]+', rule['version']):
                raise ValueError('invalid version')
            if rule['status'] not in {'approved','proposed','superseded','retired'}:
                raise ValueError('unknown approval status')
            if rule['enforcement'] not in {'guidance','mechanical'}:
                raise ValueError('unknown enforcement mode')
            if rule['enforcement'] == 'mechanical':
                raise ValueError('mechanical rules require a future execution adapter')
            reference(directory, rule['document'], directory, markdown_only=True)
            if urlsplit(rule['document']).fragment != ident.lower():
                raise ValueError('rule id and document heading do not match')
            if rule['status'] in {'approved','superseded','retired'}:
                reference(directory, rule.get('approval'), directory, markdown_only=True)
            elif 'approval' in rule:
                raise ValueError('proposed rule must not carry approval')
        except (OSError, UnicodeError, ValueError) as exc:
            errors.append(f'rule[{index}]: {exc}')
    for path in sorted(directory.rglob('*.md')):
        try:
            data = path.read_bytes()
            text = data.decode('utf-8')
            if b'\r' in data or not data.endswith(b'\n') or not re.match(r'^# +\S', text):
                raise ValueError('require UTF-8/LF, H1 and final newline')
            if any(line.rstrip() != line for line in text.splitlines()):
                raise ValueError('trailing whitespace')
            opened = None
            for fence in re.findall(r'^\s*(`{3,}|~{3,})[^\n]*$', text, re.MULTILINE):
                if opened is None:
                    opened = fence
                elif fence[0] == opened[0] and len(fence) >= len(opened):
                    opened = None
            if opened is not None:
                raise ValueError('unclosed code fence')
            for value in re.findall(r'\[[^\]]*\]\(([^)]+)\)', text):
                if urlsplit(value).scheme in {'http','https','mailto'}:
                    continue
                reference(path.parent, value, root)
        except (OSError, UnicodeError, ValueError) as exc:
            errors.append(f'{path.relative_to(root)}: {exc}')
    return errors


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=Path(__file__).resolve().parents[1])
    errors = check(parser.parse_args().root)
    print(json.dumps({'status':'FAIL' if errors else 'PASS', 'scope':'rule_registry_structure_only',
                      'errors':errors, 'limits':['no semantic rule validation','no approval authentication','no product verification']}, ensure_ascii=False, indent=2))
    return bool(errors)


if __name__ == '__main__':
    raise SystemExit(main())
