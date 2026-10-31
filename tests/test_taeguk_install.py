import hashlib
import importlib.util
import json
import os
import shlex
import shutil
import subprocess
import sysconfig
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
        for key, (data, _) in bundle.items():
            if key.startswith('.claude/agents/'):
                head = data.decode().split('---')[1]
                self.assertNotIn('model:', head, key)
                self.assertNotIn('effort:', head, key)
        for pinned in ({'model': 'x'}, {'model_reasoning_effort': 'low'}):
            with self.assertRaises(ValueError):
                install.claude_agent({'name': 'taeguk_x', 'description': 'd', 'developer_instructions': 'i', **pinned})

    def test_codex_project_bundle_is_unchanged_source(self):
        bundle = install.project_bundle('codex', SOURCE)
        path = SOURCE/'codex/skills/taeguk-work-contract/SKILL.md'
        self.assertEqual(bundle['.agents/skills/taeguk-work-contract/SKILL.md'][0], path.read_bytes())

    def test_init_skill_names_source_and_agent(self):
        claude = install.init_bundle('claude', SOURCE)[install.INIT['claude']][0].decode()
        codex = install.init_bundle('codex', SOURCE)[install.INIT['codex']][0].decode()
        self.assertIn('disable-model-invocation: true', claude)
        self.assertNotIn('disable-model-invocation', codex)
        self.assertIn(f'python3 {SOURCE}/scripts/taeguk_install.py project', claude)
        self.assertIn('--agent claude', claude)
        self.assertNotIn('{{', claude + codex)


    def test_init_command_quotes_installer_path(self):
        with tempfile.TemporaryDirectory() as tmp:
            source = Path(tmp)/'taeguk $(touch pwned) "x"'
            (source/'installer').mkdir(parents=True)
            (source/'installer/taeguk-init.md').write_bytes((SOURCE/'installer/taeguk-init.md').read_bytes())
            text = install.init_bundle('claude', source)[install.INIT['claude']][0].decode()
        command = next(line for line in text.splitlines() if 'project --root' in line).split('`')[1]
        self.assertEqual(shlex.split(command)[1], str(source/'scripts/taeguk_install.py'))


    def test_git_ignored_files_are_not_bundled(self):
        with tempfile.TemporaryDirectory() as tmp:
            source = Path(tmp).resolve()
            for name in ('codex', 'docs/rules', 'installer'):
                shutil.copytree(SOURCE/name, source/name, ignore=shutil.ignore_patterns('__pycache__'))
            (source/'.gitignore').write_text('__pycache__/\n*.pyc\n')
            subprocess.run(['git', 'init', '-q', str(source)], check=True, env=install.git_env())
            cache = source/'codex/skills/taeguk-plan-cross-verification/scripts/__pycache__'
            cache.mkdir()
            (cache/'run_exclusive.cpython-313.pyc').write_bytes(b'bytecode')
            (source/'docs/rules/local.pyc').write_bytes(b'bytecode')
            bundle = {**install.project_bundle('codex', source), **install.rules_bundle(source)}
        self.assertFalse([k for k in bundle if '__pycache__' in k or k.endswith('.pyc')])
        self.assertIn('.agents/skills/taeguk-plan-cross-verification/scripts/run_exclusive.py', bundle)

    def test_crlf_source_installs_lf_text(self):
        with tempfile.TemporaryDirectory() as tmp:
            source = Path(tmp).resolve()
            for name in ('codex', 'docs/rules', 'installer'):
                shutil.copytree(SOURCE/name, source/name, ignore=shutil.ignore_patterns('__pycache__'))
            for path in list(source.rglob('*.md')) + list(source.rglob('*.toml')):
                path.write_bytes(path.read_bytes().replace(b'\n', b'\r\n'))
            crlf = {**install.project_bundle('codex', source), **install.rules_bundle(source)}
        # The unpacked copy has no commit to pin rule links to; leave the checkout's links on main too.
        lf = {**install.project_bundle('codex', SOURCE), **install.rules_bundle(SOURCE, link_pin='')}
        self.assertEqual(crlf, lf)

    def test_git_ignored_skill_directory_is_skipped_before_validation(self):
        with tempfile.TemporaryDirectory() as tmp:
            source = Path(tmp).resolve()
            for name in ('codex', 'docs/rules', 'installer'):
                shutil.copytree(SOURCE/name, source/name, ignore=shutil.ignore_patterns('__pycache__'))
            (source/'.gitignore').write_text('codex/skills/taeguk-local/\n')
            subprocess.run(['git', 'init', '-q', str(source)], check=True, env=install.git_env())
            (source/'codex/skills/taeguk-local').mkdir()
            (source/'codex/skills/taeguk-local/notes.txt').write_text('scratch')
            bundle = install.project_bundle('claude', source)
        self.assertFalse([key for key in bundle if 'taeguk-local' in key])

    def test_ignored_cache_with_symlink_does_not_block_bundle(self):
        with tempfile.TemporaryDirectory() as tmp:
            source = Path(tmp).resolve()
            for name in ('codex', 'docs/rules', 'installer'):
                shutil.copytree(SOURCE/name, source/name, ignore=shutil.ignore_patterns('__pycache__'))
            (source/'.gitignore').write_text('.cache/\n')
            subprocess.run(['git', 'init', '-q', str(source)], check=True, env=install.git_env())
            cache = source/'codex/skills/taeguk-work-contract/.cache'
            cache.mkdir()
            (cache/'link').symlink_to(source)
            bundle = install.project_bundle('codex', source)
        self.assertFalse([key for key in bundle if '.cache' in key])

    def test_archive_inside_another_repository_is_unknown(self):
        with tempfile.TemporaryDirectory() as tmp:
            outer = Path(tmp).resolve()
            env = {**install.git_env(), 'GIT_AUTHOR_NAME': 't', 'GIT_AUTHOR_EMAIL': 't@t', 'GIT_COMMITTER_NAME': 't', 'GIT_COMMITTER_EMAIL': 't@t'}
            subprocess.run(['git', 'init', '-q', str(outer)], check=True, env=env)
            subprocess.run(['git', '-C', str(outer), 'commit', '-q', '--allow-empty', '-m', 'outer'], check=True, env=env)
            vendored = outer/'vendor/taeguk'
            vendored.mkdir(parents=True)
            self.assertEqual(install.source_version(vendored), 'unknown')
            self.assertIsNone(install.published_commit(vendored))

    def test_ignored_link_does_not_hide_its_tracked_target(self):
        with tempfile.TemporaryDirectory() as tmp:
            source = Path(tmp).resolve()
            for name in ('codex', 'docs/rules', 'installer'):
                shutil.copytree(SOURCE/name, source/name, ignore=shutil.ignore_patterns('__pycache__'))
            (source/'.gitignore').write_text('alias\n')
            subprocess.run(['git', 'init', '-q', str(source)], check=True, env=install.git_env())
            skill = source/'codex/skills/taeguk-work-contract'
            (skill/'alias').symlink_to(skill/'references', target_is_directory=True)
            bundle = install.project_bundle('codex', source)
        self.assertIn('.agents/skills/taeguk-work-contract/references/completion-review.md', bundle)
        self.assertFalse([key for key in bundle if '/alias' in key])

    @unittest.skipIf(hasattr(os, 'geteuid') and os.geteuid() == 0, 'root can read a mode-000 directory')
    def test_unreadable_source_directory_fails(self):
        with tempfile.TemporaryDirectory() as tmp:
            source = Path(tmp).resolve()
            for name in ('codex', 'docs/rules', 'installer'):
                shutil.copytree(SOURCE/name, source/name, ignore=shutil.ignore_patterns('__pycache__'))
            references = source/'codex/skills/taeguk-work-contract/references'
            references.chmod(0)
            try:
                with self.assertRaises(ValueError):
                    install.project_bundle('codex', source)
            finally:
                references.chmod(0o755)

    def test_surrounding_repository_ignores_do_not_filter_source(self):
        with tempfile.TemporaryDirectory() as tmp:
            outer = Path(tmp).resolve()
            subprocess.run(['git', 'init', '-q', str(outer)], check=True, env=install.git_env())
            (outer/'.gitignore').write_text('*.md\n')
            source = outer/'vendor/taeguk'
            for name in ('codex', 'docs/rules', 'installer'):
                shutil.copytree(SOURCE/name, source/name, ignore=shutil.ignore_patterns('__pycache__'))
            bundle = {**install.project_bundle('claude', source), **install.rules_bundle(source)}
        self.assertIn('.claude/skills/taeguk-work-contract/SKILL.md', bundle)
        self.assertIn('.taeguk/rules/index.md', bundle)

    def test_symlinked_walk_root_is_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            source = Path(tmp).resolve()/'source'
            for name in ('codex', 'docs/rules', 'installer'):
                shutil.copytree(SOURCE/name, source/name, ignore=shutil.ignore_patterns('__pycache__'))
            outside = Path(tmp).resolve()/'outside-agents'
            shutil.move(str(source/'codex/agents'), outside)
            (source/'codex/agents').symlink_to(outside, target_is_directory=True)
            with self.assertRaises(ValueError):
                install.project_bundle('codex', source)

    def test_relative_cargo_target_dir_follows_source_root(self):
        calls = []
        def fake_run(args, **kwargs):
            calls.append(kwargs)
            raise OSError('no cargo')
        with tempfile.TemporaryDirectory() as tmp:
            cwd = os.getcwd()
            os.chdir(tmp)
            try:
                with patch.object(install.subprocess, 'run', side_effect=fake_run), \
                        patch.dict(os.environ, {'CARGO_TARGET_DIR': 'relative-target'}):
                    target = install.cargo_target_dir(SOURCE)
            finally:
                os.chdir(cwd)
        self.assertEqual(target, SOURCE/'relative-target')
        self.assertEqual(Path(calls[0]['env']['CARGO_TARGET_DIR']), SOURCE/'relative-target')

    def test_local_skill_with_ignored_marker_is_left_out(self):
        with tempfile.TemporaryDirectory() as tmp:
            source = Path(tmp).resolve()
            for name in ('codex', 'docs/rules', 'installer'):
                shutil.copytree(SOURCE/name, source/name, ignore=shutil.ignore_patterns('__pycache__'))
            (source/'.gitignore').write_text('codex/skills/taeguk-local/SKILL.md\n')
            subprocess.run(['git', 'init', '-q', str(source)], check=True, env=install.git_env())
            local = source/'codex/skills/taeguk-local'
            local.mkdir()
            (local/'SKILL.md').write_text('---\nname: taeguk-local\ndescription: x\n---\n')
            (local/'notes.txt').write_text('notes')
            bundle = install.project_bundle('codex', source)
        self.assertFalse([key for key in bundle if 'taeguk-local' in key])

    def test_unreadable_git_checkout_is_not_treated_as_archive(self):
        with tempfile.TemporaryDirectory() as tmp:
            source = Path(tmp).resolve()
            (source/'.git').write_text('gitdir: /nonexistent\n')
            with self.assertRaises(ValueError):
                install.own_repository(source)

    def test_claude_role_name_collision_is_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            source = Path(tmp).resolve()
            agents = source/'codex/agents'
            agents.mkdir(parents=True)
            for name in ('taeguk_a-b', 'taeguk_a_b'):
                (agents/f'{name}.toml').write_text(f'name = "{name}"\ndescription = "d"\ndeveloper_instructions = "i"\n')
            with self.assertRaises(ValueError) as caught:
                install.project_bundle('claude', source)
        self.assertIn('roles collide', str(caught.exception))

    def test_symlinked_cargo_target_is_skipped(self):
        with tempfile.TemporaryDirectory() as tmp:
            source = Path(tmp)/'source'
            crate = source/'tools/taeguk-cli'
            cache = Path(tmp)/'cache'
            (cache/'release').mkdir(parents=True)
            (cache/'release'/('taeguk' + install.EXE)).write_bytes(b'cli')
            (crate/'src').mkdir(parents=True)
            main = crate/'src/main.rs'
            main.write_text('fn main() {}')
            os.utime(main, (1_000_000, 1_000_000))
            (crate/'target').symlink_to(cache, target_is_directory=True)
            with patch.object(install, 'cargo_target_dir', return_value=crate/'target'):
                self.assertEqual(install.cli_bundle(source), {install.CLI: (b'cli', 0o755)})

    def test_failed_ignore_listing_stops_bundle(self):
        real = subprocess.run
        def failing(args, **kwargs):
            if 'ls-files' in args:
                return subprocess.CompletedProcess(args, 128, b'', b'fatal: index file corrupt')
            return real(args, **kwargs)
        with patch.object(install.subprocess, 'run', side_effect=failing):
            with self.assertRaises(ValueError):
                install.project_bundle('codex', SOURCE)

    def test_cargo_metadata_runs_from_the_crate(self):
        calls = []
        def fake_run(args, **kwargs):
            calls.append((args, kwargs))
            raise OSError('no cargo')
        with patch.object(install.subprocess, 'run', side_effect=fake_run):
            install.cargo_target_dir(SOURCE)
        self.assertEqual(calls[0][1].get('cwd'), SOURCE/'tools/taeguk-cli')

    def test_generated_artifacts_are_dropped_without_git(self):
        with tempfile.TemporaryDirectory() as tmp:
            source = Path(tmp).resolve()
            for name in ('codex', 'docs/rules', 'installer'):
                shutil.copytree(SOURCE/name, source/name, ignore=shutil.ignore_patterns('__pycache__'))
            cache = source/'codex/skills/taeguk-plan-cross-verification/scripts/__pycache__'
            cache.mkdir()
            (cache/'run_exclusive.cpython-313.pyc').write_bytes(b'bytecode')
            (source/'docs/rules/.DS_Store').write_bytes(b'finder')
            self.assertFalse(install.own_repository(source))
            bundle = {**install.project_bundle('codex', source), **install.rules_bundle(source)}
        self.assertFalse([k for k in bundle if '__pycache__' in k or k.endswith(('.pyc', '.DS_Store'))])

    def test_junction_walk_root_is_rejected(self):
        real = install.engine.is_link
        target = SOURCE/'codex/agents'
        with patch.object(install.engine, 'is_link', side_effect=lambda path: path == target or real(path)):
            with self.assertRaises(ValueError):
                install.project_bundle('codex', SOURCE)

    def test_git_ignored_roles_are_not_bundled(self):
        with tempfile.TemporaryDirectory() as tmp:
            source = Path(tmp).resolve()
            for name in ('codex', 'docs/rules', 'installer'):
                shutil.copytree(SOURCE/name, source/name, ignore=shutil.ignore_patterns('__pycache__'))
            (source/'.gitignore').write_text('taeguk_local.toml\n')
            subprocess.run(['git', 'init', '-q', str(source)], check=True, env=install.git_env())
            (source/'codex/agents/taeguk_local.toml').write_text('name = "taeguk_local"\ndescription = "d"\ndeveloper_instructions = "i"\n')
            bundle = install.project_bundle('codex', source)
        self.assertNotIn('.codex/agents/taeguk_local.toml', bundle)
        self.assertIn('.codex/agents/taeguk_designer.toml', bundle)

    def test_stale_release_cli_is_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            source = Path(tmp)
            binary = source/'tools/taeguk-cli/target/release'/('taeguk' + install.EXE)
            binary.parent.mkdir(parents=True)
            binary.write_bytes(b'cli')
            main = source/'tools/taeguk-cli/src/main.rs'
            main.parent.mkdir(parents=True)
            main.write_text('fn main() {}')
            os.utime(binary, (1_000_000, 1_000_000))
            with self.assertRaises(ValueError):
                install.cli_bundle(source)
            os.utime(binary, None)
            os.utime(main, (1_000_000, 1_000_000))
            self.assertEqual(install.cli_bundle(source), {install.CLI: (b'cli', 0o755)})

    def test_rule_links_follow_installed_commit(self):
        commit = 'a' * 40
        with patch.object(install, 'published_commit', return_value=commit):
            pinned = install.rules_bundle(SOURCE)['.taeguk/rules/approvals.md'][0]
        with patch.object(install, 'published_commit', return_value=None):
            unpinned = install.rules_bundle(SOURCE)['.taeguk/rules/approvals.md'][0]
        self.assertIn(f'taeguk/blob/{commit}/docs/research'.encode(), pinned)
        self.assertNotIn(b'taeguk/blob/main/', pinned)
        self.assertIn(b'taeguk/blob/main/docs/research', unpinned)

    def test_pin_uses_newest_main_commit_not_branch_head(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo = Path(tmp)
            env = {**install.git_env(), 'GIT_AUTHOR_NAME': 't', 'GIT_AUTHOR_EMAIL': 't@t', 'GIT_COMMITTER_NAME': 't', 'GIT_COMMITTER_EMAIL': 't@t'}
            git = lambda *a: subprocess.run(['git', '-C', str(repo), *a], check=True, capture_output=True, text=True, env=env).stdout.strip()
            git('init', '-q', '-b', 'main')
            git('commit', '-q', '--allow-empty', '-m', 'main')
            on_main = git('rev-parse', 'HEAD')
            git('update-ref', 'refs/remotes/origin/main', on_main)
            git('checkout', '-q', '-b', 'feature')
            git('commit', '-q', '--allow-empty', '-m', 'feature')
            self.assertEqual(install.published_commit(repo), on_main)
            git('update-ref', '-d', 'refs/remotes/origin/main')
            self.assertIsNone(install.published_commit(repo))

    def test_cli_found_in_cargo_target_dir(self):
        with tempfile.TemporaryDirectory() as tmp:
            target = Path(tmp)/'elsewhere'
            binary = target/'release'/('taeguk' + install.EXE)
            binary.parent.mkdir(parents=True)
            binary.write_bytes(b'cli')
            with patch.dict(os.environ, {'CARGO_TARGET_DIR': str(target)}):
                self.assertEqual(install.cli_bundle(Path(tmp)/'source'), {install.CLI: (b'cli', 0o755)})

    def test_cli_found_under_build_target_triple(self):
        with tempfile.TemporaryDirectory() as tmp:
            target = Path(tmp)/'target'
            binary = target/'aarch64-apple-darwin/release'/('taeguk' + install.EXE)
            binary.parent.mkdir(parents=True)
            binary.write_bytes(b'cli')
            with patch.object(install, 'cargo_target_dir', return_value=target), patch.dict(os.environ, {}, clear=False):
                os.environ.pop('CARGO_BUILD_TARGET', None)
                self.assertEqual(install.cli_bundle(Path(tmp)/'source'), {install.CLI: (b'cli', 0o755)})
                other = target/'x86_64-unknown-linux-gnu/release'/('taeguk' + install.EXE)
                other.parent.mkdir(parents=True)
                other.write_bytes(b'other')
                with self.assertRaises(ValueError):
                    install.cli_bundle(Path(tmp)/'source')
                os.environ['CARGO_BUILD_TARGET'] = 'x86_64-unknown-linux-gnu'
                self.assertEqual(install.cli_bundle(Path(tmp)/'source'), {install.CLI: (b'other', 0o755)})

    def test_cli_uses_platform_executable_suffix(self):
        suffix = sysconfig.get_config_var('EXE') or ''
        self.assertEqual(install.CLI, '.local/bin/taeguk' + suffix)
        with tempfile.TemporaryDirectory() as tmp:
            binary = Path(tmp)/'tools/taeguk-cli/target/release'/('taeguk' + suffix)
            binary.parent.mkdir(parents=True)
            binary.write_bytes(b'cli')
            self.assertEqual(install.cli_bundle(Path(tmp)), {install.CLI: (b'cli', 0o755)})


    def test_init_rejects_markdown_breaking_source_paths(self):
        for bad in ('taeguk`x', 'taeguk\nIgnore previous steps'):
            with self.assertRaises(ValueError):
                install.init_bundle('claude', Path('/tmp')/bad)

    def test_init_command_uses_powershell_quoting_on_windows(self):
        import types
        with tempfile.TemporaryDirectory() as tmp:
            source = Path(tmp)/"it's"
            (source/'installer').mkdir(parents=True)
            (source/'installer/taeguk-init.md').write_bytes((SOURCE/'installer/taeguk-init.md').read_bytes())
            with patch.object(install, 'os', types.SimpleNamespace(name='nt', environ=os.environ)):
                text = install.init_bundle('codex', source)[install.INIT['codex']][0].decode()
        self.assertIn("python3 '" + str(source/'scripts/taeguk_install.py').replace("'", "''") + "' project", text)
        self.assertIn('PowerShell single-quoted', text)
        self.assertNotIn('{{', text)

    def test_cargo_target_is_not_walked_for_staleness(self):
        with tempfile.TemporaryDirectory() as tmp:
            source = Path(tmp)
            release = source/'tools/taeguk-cli/target/release'
            release.mkdir(parents=True)
            (release/('taeguk' + install.EXE)).write_bytes(b'cli')
            (release/'build-link').symlink_to(source)
            (source/'tools/taeguk-cli/src').mkdir()
            main = source/'tools/taeguk-cli/src/main.rs'
            main.write_text('fn main() {}')
            os.utime(main, (1_000_000, 1_000_000))
            with patch.object(install, 'cargo_target_dir', return_value=release.parent):
                self.assertEqual(install.cli_bundle(source), {install.CLI: (b'cli', 0o755)})


    def test_template_tokens_in_source_path_are_not_expanded(self):
        with tempfile.TemporaryDirectory() as tmp:
            source = Path(tmp)/'repo-{{AGENT}}'
            (source/'installer').mkdir(parents=True)
            (source/'installer/taeguk-init.md').write_bytes((SOURCE/'installer/taeguk-init.md').read_bytes())
            text = install.init_bundle('claude', source)[install.INIT['claude']][0].decode()
        self.assertIn('repo-{{AGENT}}/scripts/taeguk_install.py', text)
        self.assertNotIn('repo-claude', text)

    def test_custom_cargo_target_inside_crate_is_not_walked(self):
        with tempfile.TemporaryDirectory() as tmp:
            source = Path(tmp)
            crate = source/'tools/taeguk-cli'
            release = crate/'build-out/release'
            release.mkdir(parents=True)
            (release/('taeguk' + install.EXE)).write_bytes(b'cli')
            (release/'build-link').symlink_to(source)
            (crate/'src').mkdir()
            main = crate/'src/main.rs'
            main.write_text('fn main() {}')
            os.utime(main, (1_000_000, 1_000_000))
            with patch.object(install, 'cargo_target_dir', return_value=crate/'build-out'):
                self.assertEqual(install.cli_bundle(source), {install.CLI: (b'cli', 0o755)})


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

    def test_intermediate_file_blocks_every_component(self):
        (self.root/'.claude').write_text('user file')
        with self.assertRaises(ValueError):
            install.project(SOURCE, self.root, self.home, ['codex', 'claude'], 'apply')
        self.assertFalse((self.root/'.taeguk').exists())
        self.assertFalse((self.root/'.codex').exists())

    def test_agent_list_must_be_distinct_and_supported(self):
        self.assertEqual(install.parse_agents('claude,codex'), ['claude', 'codex'])
        for text in ('claude,claude', 'claude,gemini', ''):
            with self.assertRaises(ValueError):
                install.parse_agents(text)

    def test_missing_project_root_is_rejected(self):
        missing = self.root/'typo'
        with self.assertRaises(ValueError):
            install.project(SOURCE, missing, self.home, ['claude'], 'apply')
        self.assertFalse(missing.exists())

    def test_uninstall_does_not_need_current_skills(self):
        install.project(SOURCE, self.root, self.home, ['claude'], 'apply')
        with patch.object(install, 'skill_files', side_effect=ValueError('bundle has no skills')):
            result = install.project(SOURCE, self.root, self.home, ['claude'], 'uninstall')
        self.assertEqual(result['components']['claude']['status'], 'removed')

    def test_crlf_checkout_still_verifies(self):
        install.project(SOURCE, self.root, self.home, ['claude'], 'apply')
        for path in (self.root/'.claude/agents').glob('*.md'):
            path.write_bytes(path.read_bytes().replace(b'\n', b'\r\n'))
        self.assertEqual(install.project(SOURCE, self.root, self.home, ['claude'], 'check')['components']['claude']['status'], 'verified')
        install.project(SOURCE, self.root, self.home, ['claude'], 'apply')
        (self.root/'.claude/agents/taeguk-designer.md').write_text('edited')
        with self.assertRaises(ValueError):
            install.project(SOURCE, self.root, self.home, ['claude'], 'uninstall')

    def test_default_agents_follow_installed_receipts(self):
        install.project(SOURCE, self.root, self.home, ['claude'], 'apply')
        self.assertEqual(install.select_agents(None, 'check', self.root), ['claude'])
        self.assertEqual(install.select_agents(None, 'apply', self.root), list(install.AGENTS))
        agents = install.select_agents(None, 'check', self.root)
        self.assertEqual(install.project(SOURCE, self.root, self.home, agents, 'check')['components']['claude']['status'], 'verified')
        with self.assertRaises(ValueError):
            install.select_agents(None, 'uninstall', self.home, 'init-')

    def test_user_uninstall_without_cli_receipt(self):
        install.user(SOURCE, self.home, ['claude'], 'apply', with_cli=False)
        self.assertEqual(install.user(SOURCE, self.home, ['claude'], 'check')['cli'], 'not installed')
        result = install.user(SOURCE, self.home, ['claude'], 'uninstall')
        self.assertEqual(result['cli'], 'not installed')
        self.assertFalse((self.home/install.INIT['claude']).exists())

    def test_receipt_cannot_claim_unowned_project_files(self):
        install.project(SOURCE, self.root, self.home, ['claude'], 'apply')
        readme = self.root/'README.md'
        readme.write_text('project readme')
        path = self.root/install.receipt('claude')
        doc = json.loads(path.read_text())
        doc['files']['README.md'] = hashlib.sha256(b'project readme').hexdigest()
        path.write_text(json.dumps(doc))
        with self.assertRaises(ValueError):
            install.project(SOURCE, self.root, self.home, ['claude'], 'uninstall')
        self.assertEqual(readme.read_text(), 'project readme')
        self.assertTrue((self.root/'.claude/agents/taeguk-designer.md').is_file())

    def test_rules_only_project_can_be_checked_and_removed(self):
        install.project(SOURCE, self.root, self.home, [], 'apply')
        self.assertEqual(install.select_agents(None, 'uninstall', self.root), [])
        self.assertEqual(install.project(SOURCE, self.root, self.home, [], 'check')['components']['rules']['status'], 'verified')
        install.project(SOURCE, self.root, self.home, [], 'uninstall')
        self.assertFalse((self.root/'.taeguk').exists())

    def test_git_ignored_install_targets_are_reported(self):
        subprocess.run(['git', 'init', '-q', str(self.root)], check=True, env=install.git_env())
        (self.root/'.gitignore').write_text('.taeguk/\n')
        result = install.project(SOURCE, self.root, self.home, ['claude'], 'dry-run')
        self.assertIn('.taeguk/receipts/rules.json', result['ignored_by_git']['examples'])
        (self.root/'.gitignore').write_text('.taeguk/*\n!.taeguk/rules/\n!.taeguk/receipts/\n')
        self.assertNotIn('ignored_by_git', install.project(SOURCE, self.root, self.home, ['claude'], 'dry-run'))

    def test_receipt_cannot_claim_non_role_file_in_agents_dir(self):
        install.project(SOURCE, self.root, self.home, ['codex'], 'apply')
        notes = self.root/'.codex/agents/taeguk_notes.txt'
        notes.write_text('notes')
        path = self.root/install.receipt('codex')
        doc = json.loads(path.read_text())
        doc['files']['.codex/agents/taeguk_notes.txt'] = hashlib.sha256(b'notes').hexdigest()
        path.write_text(json.dumps(doc))
        with self.assertRaises(ValueError):
            install.project(SOURCE, self.root, self.home, ['codex'], 'uninstall')
        self.assertEqual(notes.read_text(), 'notes')

    def test_output_separates_installer_and_installed_versions(self):
        result = install.project(SOURCE, self.root, self.home, ['claude'], 'apply')
        self.assertEqual(result['installed_versions'], {})
        path = self.root/install.receipt('claude')
        doc = json.loads(path.read_text())
        doc['source_version'] = 'older'
        path.write_text(json.dumps(doc))
        result = install.project(SOURCE, self.root, self.home, ['claude'], 'check')
        self.assertEqual(result['installed_versions']['claude'], 'older')
        self.assertEqual(result['installer_version'], install.source_version(SOURCE))
        self.assertNotIn('source_version', result)

    def test_empty_receipt_blocks_removal_before_any_change(self):
        install.project(SOURCE, self.root, self.home, ['claude'], 'apply')
        path = self.root/install.receipt('claude')
        doc = json.loads(path.read_text())
        doc['files'] = {}
        path.write_text(json.dumps(doc))
        with self.assertRaises(ValueError):
            install.project(SOURCE, self.root, self.home, ['claude'], 'uninstall')
        self.assertTrue((self.root/'.taeguk/rules/registry.toml').is_file())

    def test_link_escaping_root_is_rejected(self):
        outside = Path(self.tmp.name)/'outside'
        outside.mkdir()
        (self.root/'.claude').symlink_to(outside, target_is_directory=True)
        with self.assertRaises(ValueError):
            install.project(SOURCE, self.root, self.home, ['claude'], 'apply')
        self.assertEqual(list(outside.iterdir()), [])

    def test_single_agent_apply_keeps_other_installed_agents_in_sync(self):
        install.project(SOURCE, self.root, self.home, ['claude', 'codex'], 'apply')
        path = self.root/install.receipt('codex')
        doc = json.loads(path.read_text())
        doc['source_version'] = 'older'
        path.write_text(json.dumps(doc))
        preview = install.project(SOURCE, self.root, self.home, ['claude'], 'dry-run')
        self.assertEqual(preview['synced_agents'], ['codex'])
        self.assertEqual(set(preview['components']), {'rules', 'claude', 'codex'})
        result = install.project(SOURCE, self.root, self.home, ['claude'], 'apply')
        self.assertEqual(result['synced_agents'], ['codex'])
        (self.root/'.codex/agents/taeguk_designer.toml').write_text('tampered')
        with self.assertRaises(ValueError):
            install.project(SOURCE, self.root, self.home, ['claude'], 'check')
        self.assertEqual(set(result['components']), {'rules', 'claude', 'codex'})
        self.assertNotEqual(json.loads(path.read_text())['source_version'], 'older')

    def test_check_reuses_recorded_link_pin(self):
        commit = 'b' * 40
        with patch.object(install, 'published_commit', return_value=commit):
            install.project(SOURCE, self.root, self.home, ['claude'], 'apply')
        self.assertEqual(json.loads((self.root/install.receipt('rules')).read_text())['link_pin'], commit)
        with patch.object(install, 'published_commit', return_value=None):
            result = install.project(SOURCE, self.root, self.home, ['claude'], 'check')
        self.assertEqual(result['components']['rules']['status'], 'verified')

    def test_partial_uninstall_keeps_shared_components(self):
        install.project(SOURCE, self.root, self.home, ['claude', 'codex'], 'apply')
        result = install.project(SOURCE, self.root, self.home, ['claude'], 'uninstall')
        self.assertEqual(result['kept'], {'rules': ['codex']})
        self.assertTrue((self.root/'.taeguk/rules/registry.toml').is_file())
        self.assertFalse((self.root/'.claude/agents/taeguk-verifier.md').exists())
        self.assertEqual(install.project(SOURCE, self.root, self.home, ['codex'], 'check')['components']['codex']['status'], 'verified')
        install.project(SOURCE, self.root, self.home, ['codex'], 'uninstall')
        self.assertFalse((self.root/'.taeguk').exists())
        install.user(SOURCE, self.home, list(install.AGENTS), 'apply')
        result = install.user(SOURCE, self.home, ['claude'], 'uninstall')
        self.assertEqual(result['kept'], {'cli': ['codex']})
        self.assertTrue((self.home/install.CLI).is_file())
        install.user(SOURCE, self.home, ['codex'], 'uninstall')
        self.assertFalse((self.home/install.CLI).exists())

    def test_uninstall_ignores_unmanaged_files_at_new_bundle_paths(self):
        install.project(SOURCE, self.root, self.home, ['claude'], 'apply')
        path = self.root/install.receipt('claude')
        doc = json.loads(path.read_text())
        new_path = '.claude/agents/taeguk-designer.md'
        del doc['files'][new_path]
        path.write_text(json.dumps(doc))
        install.project(SOURCE, self.root, self.home, ['claude'], 'uninstall')
        self.assertTrue((self.root/new_path).is_file())
        self.assertFalse((self.root/install.receipt('claude')).exists())

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
        self.files = {install.CLI: hashlib.sha256(b'cli').hexdigest(), '.codex/agents/taeguk_a.toml': '0'*64}
        self.legacy.write_text(json.dumps({'schema_version': 1, 'source_root': '/src', 'files': self.files}))

    def test_dry_run_then_apply_splits_receipt(self):
        self.assertEqual(install.migrate(self.home, 'dry-run')['status'], 'planned')
        self.assertEqual(json.loads(self.legacy.read_text())['files'], self.files)
        install.migrate(self.home, 'apply')
        self.assertEqual(json.loads(self.legacy.read_text())['files'], {'.codex/agents/taeguk_a.toml': '0'*64})
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
        agent = self.home/'.codex/agents/taeguk_a.toml'
        agent.parent.mkdir()
        agent.write_bytes(b'role')
        self.files['.codex/agents/taeguk_a.toml'] = hashlib.sha256(b'role').hexdigest()
        self.legacy.write_text(json.dumps({'schema_version': 1, 'source_root': '/src', 'files': self.files}))
        install.migrate(self.home, 'apply')
        legacy_bundle = {install.CLI: (b'cli', 0o755), '.codex/agents/taeguk_a.toml': (b'role', 0o644)}
        with patch.object(install.engine, 'payload', return_value=legacy_bundle):
            with self.assertRaises(ValueError):
                install.engine.execute(SOURCE, self.home, 'apply')
            install.engine.execute(SOURCE, self.home, 'uninstall')
        self.assertFalse(agent.exists())
        self.assertFalse(self.legacy.exists())
        self.assertEqual((self.home/install.CLI).read_bytes(), b'cli')

    def test_cli_only_user_scope_can_be_checked_and_removed(self):
        (self.home/install.CLI).chmod(0o755)
        install.migrate(self.home, 'apply')
        self.assertEqual(install.select_agents(None, 'check', self.home, 'init-'), [])
        with patch.object(install, 'cli_bundle', return_value={install.CLI: (b'cli', 0o755)}):
            self.assertEqual(install.user(SOURCE, self.home, [], 'check')['components']['cli']['status'], 'verified')
            install.user(SOURCE, self.home, [], 'uninstall')
        self.assertFalse((self.home/install.CLI).exists())

    def test_legacy_receipt_cannot_claim_unowned_home_files(self):
        secret = self.home/'.ssh/config'
        secret.parent.mkdir()
        secret.write_text('Host *')
        role = self.home/'.codex/agents/taeguk_a.toml'
        role.parent.mkdir(parents=True)
        role.write_bytes(b'role')
        self.files['.codex/agents/taeguk_a.toml'] = hashlib.sha256(b'role').hexdigest()
        self.files['.ssh/config'] = hashlib.sha256(b'Host *').hexdigest()
        self.legacy.write_text(json.dumps({'schema_version': 1, 'source_root': '/src', 'files': self.files}))
        with self.assertRaises(ValueError):
            install.engine.execute(SOURCE, self.home, 'uninstall')
        self.assertEqual(secret.read_text(), 'Host *')

    def test_no_cli_check_still_verifies_managed_cli(self):
        (self.home/install.CLI).chmod(0o755)
        install.migrate(self.home, 'apply')
        result = install.user(SOURCE, self.home, [], 'check', with_cli=False)
        self.assertIn('matches its receipt', result['cli'])
        (self.home/install.CLI).chmod(0o644)
        with self.assertRaises(ValueError):
            install.user(SOURCE, self.home, [], 'check', with_cli=False)
        (self.home/install.CLI).chmod(0o755)
        (self.home/install.CLI).write_bytes(b'tampered')
        with self.assertRaises(ValueError):
            install.user(SOURCE, self.home, [], 'check', with_cli=False)

    def test_migrate_rejects_legacy_receipt_with_unowned_paths(self):
        self.files['.ssh/config'] = '0' * 64
        self.legacy.write_text(json.dumps({'schema_version': 1, 'source_root': '/src', 'files': self.files}))
        with self.assertRaises(ValueError):
            install.migrate(self.home, 'apply')
        self.assertFalse((self.home/install.receipt('cli')).exists())

    def test_migrated_cli_can_be_updated_by_user_scope(self):
        install.migrate(self.home, 'apply')
        with patch.object(install, 'cli_bundle', return_value={install.CLI: (b'new cli', 0o755)}):
            install.user(SOURCE, self.home, ['claude'], 'apply')
        self.assertEqual((self.home/install.CLI).read_bytes(), b'new cli')


class CommandLineTest(unittest.TestCase):
    def test_command_without_mode_flag_is_dry_run(self):
        with tempfile.TemporaryDirectory() as tmp:
            home, root = Path(tmp)/'home', Path(tmp)/'project'
            home.mkdir()
            root.mkdir()
            run = subprocess.run([sys.executable, str(SOURCE/'scripts/taeguk_install.py'), '--home', str(home),
                                  'project', '--root', str(root), '--agent', 'claude,codex'],
                                 capture_output=True, text=True)
            self.assertEqual(run.returncode, 0, run.stderr)
            result = json.loads(run.stdout)
            self.assertEqual({c['status'] for c in result['components'].values()}, {'planned'})
            self.assertEqual([p for p in Path(tmp).rglob('*') if p.is_file()], [])


if __name__ == '__main__':
    unittest.main()
