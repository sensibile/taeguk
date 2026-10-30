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


def run(text, *args):
    with tempfile.NamedTemporaryFile('w', suffix='.md', encoding='utf-8', delete=False) as handle:
        handle.write(text)
    try:
        result = subprocess.run([sys.executable, '-I', str(SCRIPT), *args, handle.name],
                                capture_output=True, text=True, encoding='utf-8')
    finally:
        Path(handle.name).unlink()
    return result.returncode, json.loads(result.stdout)


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


if __name__ == '__main__':
    unittest.main()
