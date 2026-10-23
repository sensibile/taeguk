import importlib.util
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

SOURCE = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SOURCE / 'scripts'))
spec = importlib.util.spec_from_file_location('install_claude', SOURCE / 'scripts/install_claude.py')
install = importlib.util.module_from_spec(spec)
spec.loader.exec_module(install)


class ClaudeBundleTest(unittest.TestCase):
    def setUp(self):
        self.bundle = install.payload(SOURCE)

    def test_bundle_covers_every_role_and_skill(self):
        agents = sorted(p.stem.replace('_', '-') for p in (SOURCE/'codex/agents').glob('taeguk_*.toml'))
        skills = sorted(p.name for p in (SOURCE/'codex/skills').glob('taeguk-*'))
        self.assertEqual(sorted(Path(k).stem for k in self.bundle if k.startswith('.claude/agents/')), agents)
        self.assertEqual(sorted({k.split('/')[2] for k in self.bundle if k.startswith('.claude/skills/')}), skills)
        self.assertTrue(all(k.startswith('.claude/') for k in self.bundle))

    def test_codex_only_files_and_syntax_are_excluded(self):
        self.assertFalse([k for k in self.bundle if '/agents/openai.yaml' in k])
        for key, (data, _) in self.bundle.items():
            if key.endswith('.md'):
                self.assertNotIn(b'$taeguk-', data, key)

    def test_agent_frontmatter_maps_models(self):
        text = self.bundle['.claude/agents/taeguk-designer.md'][0].decode()
        head = text.split('---')[1]
        self.assertIn('name: taeguk-designer\n', head)
        self.assertIn('model: opus\n', head)
        self.assertIn('effort: medium\n', head)
        implementer = self.bundle['.claude/agents/taeguk-implementer.md'][0].decode().split('---')[1]
        self.assertIn('model: sonnet\n', implementer)
        self.assertNotIn('effort:', implementer)
        coordinator = self.bundle['.claude/agents/taeguk-coordinator.md'][0].decode()
        self.assertNotIn('model:', coordinator.split('---')[1])
        self.assertIn('/taeguk-work-contract', coordinator)

    def test_unmapped_model_is_rejected(self):
        with self.assertRaises(ValueError):
            install.agent_markdown({'name': 'taeguk_x', 'description': 'd',
                                    'developer_instructions': 'i', 'model': 'unknown'})


class ClaudeInstallTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.home = Path(self.tmp.name)
        self.addCleanup(self.tmp.cleanup)

    def test_install_check_uninstall_preserves_unrelated_files(self):
        settings = self.home/'.claude/settings.json'
        settings.parent.mkdir()
        settings.write_text('{}')
        codex_receipt = self.home/'.codex/taeguk-install.json'
        codex_receipt.parent.mkdir()
        codex_receipt.write_text('codex')
        self.assertEqual(install.execute(SOURCE, self.home)['status'], 'planned')
        self.assertFalse((self.home/install.RECEIPT).exists())
        install.execute(SOURCE, self.home, 'apply')
        result = install.execute(SOURCE, self.home, 'check')
        self.assertEqual((result['status'], result['cli']), ('verified', 'missing'))
        install.execute(SOURCE, self.home, 'uninstall')
        self.assertEqual(settings.read_text(), '{}')
        self.assertEqual(codex_receipt.read_text(), 'codex')
        self.assertFalse((self.home/'.claude/agents/taeguk-designer.md').exists())

    def test_unmanaged_skill_collision_blocks_install(self):
        target = self.home/'.claude/skills/taeguk-work-contract/SKILL.md'
        target.parent.mkdir(parents=True)
        target.write_text('user skill')
        with self.assertRaises(ValueError):
            install.execute(SOURCE, self.home, 'apply')
        self.assertEqual(target.read_text(), 'user skill')
        self.assertFalse((self.home/install.RECEIPT).exists())

    def test_codex_installer_default_still_uses_its_own_payload(self):
        with patch.object(install.install_codex, 'payload', return_value={'.codex/agents/a.toml': (b'x', 0o644)}):
            install.install_codex.execute(SOURCE, self.home, 'apply')
        self.assertTrue((self.home/install.install_codex.RECEIPT).exists())
        self.assertFalse((self.home/install.RECEIPT).exists())


if __name__ == '__main__':
    unittest.main()
