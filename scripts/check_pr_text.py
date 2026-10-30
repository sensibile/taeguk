#!/usr/bin/env python3
"""Check PR titles, bodies and comments for a Codex mention before they are posted.

A mention anywhere in the text, code spans included, makes the GitHub Codex integration start a
cloud task instead of a review (sensibile/taeguk#13 to #16). What starts the task is not known
beyond that, so anything after the name (Hangul, a hyphen, more letters) still fails; only an ASCII
name character before it (an email address) is skipped. This checks text only; it cannot see what
is posted without it. A missing, unreadable or empty input is UNKNOWN, not PASS.
"""
import argparse
import json
import re
import sys

# An email such as ops@codex.example is skipped; @codex-bot or @codexreview fails on purpose.
MENTION = re.compile(r'(?<![A-Za-z0-9_])@codex', re.IGNORECASE)
REVIEW_REQUEST = '@codex review'
FIX = 'write "Codex 리뷰 요청" or "Codex review" without the @ instead'


def findings(text, allow_review_request=False):
    if allow_review_request and text.strip() == REVIEW_REQUEST:
        return []
    # Only the line number: a drafted line can hold a token, and this output lands in agent logs.
    return [{'line': number}
            for number, line in enumerate(text.split('\n'), 1) if MENTION.search(line)]


def read(name):
    if name == '-':
        return sys.stdin.read()
    with open(name, encoding='utf-8') as handle:
        return handle.read()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('files', nargs='*', help='text files to check; stdin when none or "-"')
    parser.add_argument('--allow-review-request', action='store_true',
                        help=f'accept a file whose whole text is "{REVIEW_REQUEST}"; '
                             'pass it only when checking one review-request comment')
    args = parser.parse_args()
    if args.allow_review_request and len(args.files) > 1:
        parser.error('--allow-review-request checks one comment at a time')
    found, errors = [], []
    for name in args.files or ['-']:
        try:
            text = read(name)
        except (OSError, UnicodeDecodeError) as error:
            errors.append({'file': name, 'error': str(error)})
            continue
        if not text.strip():
            errors.append({'file': name, 'error': 'empty input'})
            continue
        found += [{'file': name, **item} for item in findings(text, args.allow_review_request)]
    status = 'FAIL' if found else 'UNKNOWN' if errors else 'PASS'
    print(json.dumps({'status': status, 'findings': found, 'errors': errors,
                      'fix': FIX if found else None}, ensure_ascii=False, indent=2))
    return {'PASS': 0, 'FAIL': 1, 'UNKNOWN': 2}[status]


if __name__ == '__main__':
    raise SystemExit(main())
