#!/usr/bin/env python3
"""Check PR titles, bodies and comments for a Codex mention before they are posted.

A mention anywhere in the text, code spans included, makes the GitHub Codex integration start a
cloud task instead of a review (sensibile/taeguk#13 to #16). This checks text only; it cannot see
what is posted without it.
"""
import argparse
import json
import re
import sys

MENTION = re.compile(r'(?<![\w.+-])@codex\b', re.IGNORECASE)
REVIEW_REQUEST = '@codex review'
FIX = 'write "Codex 리뷰 요청" or "Codex review" without the @ instead'


def findings(text, allow_review_request=False):
    if allow_review_request and text.strip() == REVIEW_REQUEST:
        return []
    return [{'line': number, 'text': line.strip()}
            for number, line in enumerate(text.splitlines(), 1) if MENTION.search(line)]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('files', nargs='*', help='text files to check; stdin when none')
    parser.add_argument('--allow-review-request', action='store_true',
                        help=f'accept a comment whose whole text is "{REVIEW_REQUEST}"')
    args = parser.parse_args()
    sources = [(name, open(name, encoding='utf-8').read()) for name in args.files] or [('-', sys.stdin.read())]
    found = [{'file': name, **item} for name, text in sources for item in findings(text, args.allow_review_request)]
    print(json.dumps({'status': 'FAIL' if found else 'PASS', 'findings': found,
                      'fix': FIX if found else None}, ensure_ascii=False, indent=2))
    return bool(found)


if __name__ == '__main__':
    raise SystemExit(main())
