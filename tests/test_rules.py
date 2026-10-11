import importlib.util
from pathlib import Path
import shutil
import subprocess
import sys
import json
import tempfile
import unittest

SOURCE = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('check_rules', SOURCE/'scripts/check_rules.py')
checker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(checker)


class RuleCheckTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        shutil.copytree(SOURCE/'docs', self.root/'docs')
        self.registry = self.root/'docs/rules/registry.toml'

    def test_current_registry_passes(self):
        self.assertEqual(checker.check(self.root), [])

    def test_missing_approval_blocks_approved_rule(self):
        self.registry.write_text(self.registry.read_text().replace('approval = "approvals.md#approval-2026-10-05"', '', 1))
        self.assertTrue(any('reference' in x for x in checker.check(self.root)))

    def test_duplicate_id_fails(self):
        self.registry.write_text(self.registry.read_text().replace('id = "GOV-002"', 'id = "GOV-001"'))
        self.assertTrue(any('duplicate' in x for x in checker.check(self.root)))

    def test_wrong_heading_fails(self):
        self.registry.write_text(self.registry.read_text().replace('governance.md#gov-001', 'governance.md#missing'))
        self.assertTrue(any('anchor' in x for x in checker.check(self.root)))

    def test_wrong_existing_rule_heading_fails(self):
        self.registry.write_text(self.registry.read_text().replace('governance.md#gov-001', 'governance.md#gov-002'))
        self.assertTrue(any('do not match' in x for x in checker.check(self.root)))

    def test_cli_failure_has_nonzero_exit(self):
        self.registry.write_text('schema_version = 99\n')
        result = subprocess.run([sys.executable, SOURCE/'scripts/check_rules.py', '--root', self.root], capture_output=True, text=True)
        self.assertEqual(result.returncode, 1)
        self.assertEqual(json.loads(result.stdout)['status'], 'FAIL')

    def test_invalid_status_fails(self):
        self.registry.write_text(self.registry.read_text().replace('status = "approved"', 'status = "passed"', 1))
        self.assertTrue(any('status' in x for x in checker.check(self.root)))

    def test_escaping_reference_fails(self):
        self.registry.write_text(self.registry.read_text().replace('governance.md#gov-001', '../../README.md'))
        self.assertTrue(any('out-of-root' in x for x in checker.check(self.root)))

    def test_semantic_enforcement_claim_fails(self):
        self.registry.write_text(self.registry.read_text().replace('enforcement = "guidance"', 'enforcement = "mechanical"', 1))
        self.assertTrue(any('adapter' in x for x in checker.check(self.root)))

    def test_proposed_with_approval_fails(self):
        self.registry.write_text(self.registry.read_text()+'approval = "approvals.md#approval-2026-10-11"\n')
        self.assertTrue(any('proposed' in x for x in checker.check(self.root)))

    def test_broken_link_and_unclosed_fence_fail(self):
        p=self.root/'docs/rules/index.md'
        p.write_text(p.read_text()+'\n[broken](missing.md)\n')
        self.assertTrue(any('reference' in x for x in checker.check(self.root)))
        p.write_text('# Test\n\n```sh\ncommand\n')
        self.assertTrue(any('fence' in x for x in checker.check(self.root)))


if __name__ == '__main__':
    unittest.main()
