import importlib.util
from pathlib import Path
import shutil
import tempfile
import unittest

SOURCE = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('check_doc_drift', SOURCE/'scripts/check_doc_drift.py')
drift = importlib.util.module_from_spec(spec)
spec.loader.exec_module(drift)


class DocDriftTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        shutil.copytree(SOURCE/'docs', self.root/'docs')
        shutil.copytree(SOURCE/'codex', self.root/'codex')
        (self.root/'scripts').mkdir()
        (self.root/'scripts/check_rules.py').write_text('')

    def write(self, rel, text):
        path = self.root/rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding='utf-8')

    def kinds(self):
        return {(f['kind'], f['file'], f['value']) for f in drift.scan(self.root)}

    def test_clean_document_has_no_findings(self):
        self.write('docs/note.md', '# Note\n\n[ops](repository-operations.md), `$taeguk-test-gap`, `scripts/check_rules.py`\n')
        self.assertNotIn('docs/note.md', {f['file'] for f in drift.scan(self.root)})

    def test_missing_link_target(self):
        self.write('docs/note.md', '# Note\n\n[gone](missing.md#part)\n')
        self.assertIn(('link', 'docs/note.md', 'missing.md#part'), self.kinds())

    def test_unknown_skill_but_not_generated_init(self):
        self.write('docs/note.md', '# Note\n\nUse `$taeguk-removed-skill` then `$taeguk-init`.\n')
        found = self.kinds()
        self.assertIn(('skill', 'docs/note.md', 'taeguk-removed-skill'), found)
        self.assertNotIn(('skill', 'docs/note.md', 'taeguk-init'), found)

    def test_rule_version_differs_from_registry(self):
        self.write('docs/note.md', '# Note\n\nFollows VER-005 9.9.\n')
        self.assertIn(('rule_version', 'docs/note.md', 'VER-005 9.9'), self.kinds())

    def test_missing_repository_path_in_code_span(self):
        self.write('docs/note.md', '# Note\n\nRun `scripts/removed.py`.\n')
        self.assertIn(('path', 'docs/note.md', 'scripts/removed.py'), self.kinds())

    def test_rules_and_archive_are_out_of_scope(self):
        self.write('docs/rules/extra.md', '# Extra\n\n[gone](missing.md)\n')
        self.write('docs/research/archive/old.md', '# Old\n\n[gone](missing.md)\n')
        self.assertFalse({f['file'] for f in drift.scan(self.root)} & {'docs/rules/extra.md', 'docs/research/archive/old.md'})


if __name__ == '__main__':
    unittest.main()
