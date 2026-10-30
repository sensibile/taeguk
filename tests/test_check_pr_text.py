import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

SCRIPT = Path(__file__).resolve().parents[1]/'scripts/check_pr_text.py'

# Lines from the bodies that started Codex cloud tasks: sensibile/taeguk#13 and the first revision of #16.
PR13_LINE = '3. **재요청**: 기준 시점에서 10분 이상 지나도 판정이 없으면 `@codex review`를 남긴다.'
PR16_LINE = '- `scripts/codex_review_gate.py`: 시간 기준 규칙(10분 간격 재요청, 3회 무응답 통과), `@codex review` 요청, auto-merge와 draft/`no-auto-merge` opt-out 처리 제거.'
CLEAN = '## 로컬 리뷰\n\nCodex 리뷰 요청 없이 push로 자동 리뷰가 돈다. 연락처 ops@codex.example 은 멘션이 아니다.\n'


def invoke(*args, stdin=None):
    result = subprocess.run([sys.executable, '-I', str(SCRIPT), *args], input=stdin,
                            capture_output=True, text=True, encoding='utf-8')
    return result.returncode, json.loads(result.stdout)


def run(text, *args):
    with tempfile.NamedTemporaryFile('w', suffix='.md', encoding='utf-8', delete=False) as handle:
        handle.write(text)
    try:
        return invoke(*args, handle.name)
    finally:
        Path(handle.name).unlink()


class CheckPrTextTest(unittest.TestCase):
    def test_past_mentions_fail_with_the_fix(self):
        for line in (PR13_LINE, PR16_LINE):
            code, report = run(f'## 요약\n\n{line}\n')
            self.assertEqual(code, 1)
            self.assertEqual(report['status'], 'FAIL')
            self.assertEqual(report['findings'][0]['line'], 3)
            self.assertIn('Codex 리뷰 요청', report['fix'])

    def test_case_and_code_span_do_not_hide_a_mention(self):
        code, report = run('`@CODEX please`\n')
        self.assertEqual((code, report['status']), (1, 'FAIL'))

    def test_text_without_mention_passes(self):
        code, report = run(CLEAN)
        self.assertEqual((code, report['status'], report['findings']), (0, 'PASS', []))

    def test_review_request_comment_is_allowed_only_alone_and_only_with_the_flag(self):
        self.assertEqual(run('@codex review\n', '--allow-review-request')[0], 0)
        self.assertEqual(run('@codex review\n')[0], 1)
        self.assertEqual(run('@codex review\n\n그리고 이것도 봐줘\n', '--allow-review-request')[0], 1)

    def test_hangul_next_to_the_name_does_not_hide_a_mention(self):
        for text in ('@codex에게 넘기지 않는다\n', '리뷰어@codex 호출\n', '`@codex`로 부르지 않는다\n', '참고.@codex\n'):
            self.assertEqual(run(text)[0], 1, text)

    def test_an_email_is_not_a_mention(self):
        self.assertEqual(run('연락처 ops@codex.example\n')[0], 0)

    def test_anything_after_the_name_still_fails(self):
        for text in ('@codex-bot\n', '`@codex-review`\n', '@codex-리뷰\n', '@codexreview\n', '@codex_x\n'):
            self.assertEqual(run(text)[0], 1, text)

    def test_each_finding_names_its_file_and_line_across_files_and_crlf(self):
        with tempfile.TemporaryDirectory() as directory:
            clean, dirty = Path(directory)/'title.txt', Path(directory)/'body.md'
            clean.write_text('제목\n', encoding='utf-8')
            dirty.write_bytes('요약\r\n\r\n(@codex)\r\n'.encode())
            code, report = invoke(str(clean), str(dirty))
        self.assertEqual(code, 1)
        self.assertEqual([(item['file'], item['line']) for item in report['findings']], [(str(dirty), 3)])

    def test_stdin_is_checked(self):
        self.assertEqual(invoke(stdin='@codex\n')[0], 1)
        self.assertEqual(invoke('-', stdin='본문\n')[1]['status'], 'PASS')

    def test_missing_or_empty_input_is_unknown_not_pass(self):
        code, report = invoke('/nonexistent/body.md')
        self.assertEqual((code, report['status'], report['errors'][0]['file']), (2, 'UNKNOWN', '/nonexistent/body.md'))
        self.assertEqual(invoke(stdin='')[:1], (2,))
        self.assertEqual(run('  \n')[1]['status'], 'UNKNOWN')

    def test_a_mention_outweighs_an_unreadable_file(self):
        with tempfile.NamedTemporaryFile('w', suffix='.md', encoding='utf-8', delete=False) as handle:
            handle.write('@codex\n')
        try:
            self.assertEqual(invoke(handle.name, '/nonexistent/body.md')[1]['status'], 'FAIL')
        finally:
            Path(handle.name).unlink()


if __name__ == '__main__':
    unittest.main()
