import hashlib
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

SOURCE = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SOURCE / 'scripts'))
spec = importlib.util.spec_from_file_location('taeguk_install', SOURCE / 'scripts/taeguk_install.py')
install = importlib.util.module_from_spec(spec)
spec.loader.exec_module(install)
CLI_STUB = {install.CLI: (b'cli', 0o755)}


class BundleTest(unittest.TestCase):
    def test_project_bundles_cover_every_role_and_skill(self):
        roles = sorted(p.stem for p in (SOURCE/'codex/agents').glob('taeguk_*.toml'))
        skills = sorted(p.name for p in (SOURCE/'codex/skills').glob('taeguk-*'))
        claude = install.project_bundle('claude', SOURCE)
        codex = install.project_bundle('codex', SOURCE)
        self.assertEqual(sorted(Path(k).stem for k in claude if k.startswith('.claude/agents/')),
                         [r.replace('_', '-') for r in roles])
        self.assertEqual(sorted(Path(k).stem for k in codex if k.startswith('.codex/agents/')), roles)
        self.assertEqual(sorted({k.split('/')[2] for k in claude if '/skills/' in k}), skills)
        self.assertEqual(sorted({k.split('/')[2] for k in codex if '/skills/' in k}), skills)

    def test_claude_conversion(self):
        bundle = install.project_bundle('claude', SOURCE)
        self.assertFalse([k for k in bundle if k.endswith('/agents/openai.yaml')])
        self.assertFalse([k for k, (d, _) in bundle.items() if k.endswith('.md') and b'$taeguk-' in d])
        head = bundle['.claude/agents/taeguk-designer.md'][0].decode().split('---')[1]
        self.assertIn('model: opus\n', head)
        self.assertIn('effort: medium\n', head)
        self.assertIn('model: sonnet\n', bundle['.claude/agents/taeguk-implementer.md'][0].decode())
        self.assertNotIn('model:', bundle['.claude/agents/taeguk-coordinator.md'][0].decode().split('---')[1])
        with self.assertRaises(ValueError):
            install.claude_agent({'name': 'taeguk_x', 'description': 'd', 'developer_instructions': 'i', 'model': 'x'})

    def test_codex_project_bundle_is_unchanged_source(self):
        bundle = install.project_bundle('codex', SOURCE)
        path = SOURCE/'codex/skills/taeguk-work-contract/SKILL.md'
        self.assertEqual(bundle['.agents/skills/taeguk-work-contract/SKILL.md'][0], path.read_bytes())

    def test_init_skill_names_source_and_agent(self):
        claude = install.init_bundle('claude', SOURCE)[install.INIT['claude']][0].decode()
        codex = install.init_bundle('codex', SOURCE)[install.INIT['codex']][0].decode()
        self.assertIn('disable-model-invocation: true', claude)
        self.assertNotIn('disable-model-invocation', codex)
        self.assertIn(f'"{SOURCE}/scripts/taeguk_install.py" project', claude)
        self.assertIn('--agent claude', claude)
        self.assertNotIn('{{', claude + codex)


class ScopeTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.home = Path(self.tmp.name)/'home'
        self.root = Path(self.tmp.name)/'project'
        self.home.mkdir()
        self.root.mkdir()
        self.addCleanup(self.tmp.cleanup)
        stub = patch.object(install, 'cli_bundle', return_value=CLI_STUB)
        stub.start()
        self.addCleanup(stub.stop)

    def test_user_scope_installs_only_init_and_cli(self):
        self.assertEqual(install.user(SOURCE, self.home, ['claude'], 'dry-run')['components']['cli']['status'], 'planned')
        self.assertEqual(list(self.home.iterdir()), [])
        install.user(SOURCE, self.home, list(install.AGENTS), 'apply')
        files = sorted(str(p.relative_to(self.home)) for p in self.home.rglob('*') if p.is_file())
        self.assertEqual(files, sorted([install.CLI, install.INIT['claude'], install.INIT['codex'],
                                        '.taeguk/receipts/cli.json', '.taeguk/receipts/init-claude.json',
                                        '.taeguk/receipts/init-codex.json']))
        result = install.user(SOURCE, self.home, list(install.AGENTS), 'check')
        self.assertTrue(all(c['status'] == 'verified' for c in result['components'].values()))
        install.user(SOURCE, self.home, list(install.AGENTS), 'uninstall')
        self.assertEqual([p for p in self.home.rglob('*') if p.is_file()], [])

    def test_project_scope_is_relocatable_and_preserves_unrelated_files(self):
        settings = self.root/'.claude/settings.json'
        settings.parent.mkdir()
        settings.write_text('{}')
        install.project(SOURCE, self.root, self.home, ['claude', 'codex'], 'apply')
        self.assertTrue((self.root/'.taeguk/rules/registry.toml').is_file())
        self.assertTrue((self.root/'.claude/agents/taeguk-verifier.md').is_file())
        self.assertTrue((self.root/'.codex/agents/taeguk_verifier.toml').is_file())
        self.assertTrue((self.root/'.agents/skills/taeguk-self-improve/SKILL.md').is_file())
        for path in (self.root/'.taeguk/receipts').iterdir():
            doc = json.loads(path.read_text())
            self.assertNotIn('source_root', doc)
            self.assertNotIn(str(self.tmp.name), path.read_text())
        moved = Path(self.tmp.name)/'clone'
        self.root.rename(moved)
        result = install.project(SOURCE, moved, self.home, ['claude', 'codex'], 'check')
        self.assertEqual({c['status'] for c in result['components'].values()}, {'verified'})
        install.project(SOURCE, moved, self.home, ['claude', 'codex'], 'uninstall')
        self.assertEqual((moved/'.claude/settings.json').read_text(), '{}')
        self.assertFalse((moved/'.taeguk').exists())
        self.assertFalse((moved/'.agents/skills/taeguk-work-contract').exists())
        self.assertEqual(list((moved/'.claude/agents').iterdir()), [])

    def test_collision_in_later_component_blocks_every_component(self):
        target = self.root/'.agents/skills/taeguk-work-contract/SKILL.md'
        target.parent.mkdir(parents=True)
        target.write_text('user skill')
        with self.assertRaises(ValueError):
            install.project(SOURCE, self.root, self.home, ['claude', 'codex'], 'apply')
        self.assertEqual(target.read_text(), 'user skill')
        self.assertFalse((self.root/'.taeguk').exists())
        self.assertFalse((self.root/'.claude').exists())

    def test_user_level_skill_shadowing_is_reported(self):
        (self.home/'.claude/skills/taeguk-work-contract').mkdir(parents=True)
        result = install.project(SOURCE, self.root, self.home, ['claude'], 'dry-run')
        self.assertEqual(result['shadowed_by_user_skills'], {'claude': ['taeguk-work-contract']})
        self.assertNotIn('shadowed_by_user_skills', install.project(SOURCE, self.root, self.home, ['codex'], 'dry-run'))


class MigrateTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.home = Path(self.tmp.name)
        self.addCleanup(self.tmp.cleanup)
        cli = self.home/install.CLI
        cli.parent.mkdir(parents=True)
        cli.write_bytes(b'cli')
        self.legacy = self.home/install.LEGACY_RECEIPTS[0]
        self.legacy.parent.mkdir()
        self.files = {install.CLI: hashlib.sha256(b'cli').hexdigest(), '.codex/agents/a.toml': '0'*64}
        self.legacy.write_text(json.dumps({'schema_version': 1, 'source_root': '/src', 'files': self.files}))

    def test_dry_run_then_apply_splits_receipt(self):
        self.assertEqual(install.migrate(self.home, 'dry-run')['status'], 'planned')
        self.assertEqual(json.loads(self.legacy.read_text())['files'], self.files)
        install.migrate(self.home, 'apply')
        self.assertEqual(json.loads(self.legacy.read_text())['files'], {'.codex/agents/a.toml': '0'*64})
        new = json.loads((self.home/'.taeguk/receipts/cli.json').read_text())
        self.assertEqual(new['files'], {install.CLI: self.files[install.CLI]})
        self.assertEqual(install.migrate(self.home, 'apply')['status'], 'nothing to migrate')

    def test_modified_cli_blocks_migration(self):
        (self.home/install.CLI).write_bytes(b'user build')
        with self.assertRaises(ValueError):
            install.migrate(self.home, 'apply')
        self.assertEqual(json.loads(self.legacy.read_text())['files'], self.files)
        self.assertFalse((self.home/'.taeguk').exists())

    def test_legacy_uninstall_after_migration_keeps_cli(self):
        agent = self.home/'.codex/agents/a.toml'
        agent.parent.mkdir()
        agent.write_bytes(b'role')
        self.files['.codex/agents/a.toml'] = hashlib.sha256(b'role').hexdigest()
        self.legacy.write_text(json.dumps({'schema_version': 1, 'source_root': '/src', 'files': self.files}))
        install.migrate(self.home, 'apply')
        legacy_bundle = {install.CLI: (b'cli', 0o755), '.codex/agents/a.toml': (b'role', 0o644)}
        with patch.object(install.engine, 'payload', return_value=legacy_bundle):
            with self.assertRaises(ValueError):
                install.engine.execute(SOURCE, self.home, 'apply')
            install.engine.execute(SOURCE, self.home, 'uninstall')
        self.assertFalse(agent.exists())
        self.assertFalse(self.legacy.exists())
        self.assertEqual((self.home/install.CLI).read_bytes(), b'cli')

    def test_migrated_cli_can_be_updated_by_user_scope(self):
        install.migrate(self.home, 'apply')
        with patch.object(install, 'cli_bundle', return_value={install.CLI: (b'new cli', 0o755)}):
            install.user(SOURCE, self.home, ['claude'], 'apply')
        self.assertEqual((self.home/install.CLI).read_bytes(), b'new cli')


if __name__ == '__main__':
    unittest.main()
