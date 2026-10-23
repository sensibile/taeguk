import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

SOURCE = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('install_codex', SOURCE / 'scripts/install_codex.py')
install = importlib.util.module_from_spec(spec)
spec.loader.exec_module(install)


class InstallerTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.home = Path(self.tmp.name)
        self.bundle = {'.codex/agents/taeguk_test.toml': (b'new role', 0o644),
                       '.agents/skills/taeguk-test/SKILL.md': (b'new skill', 0o644),
                       '.local/bin/taeguk': (b'local cli', 0o755)}
        self.stub = patch.object(install, 'payload', return_value=self.bundle)
        self.stub.start()
        self.addCleanup(self.stub.stop)
        self.addCleanup(self.tmp.cleanup)

    def test_dry_run_does_not_write(self):
        install.execute(SOURCE, self.home)
        self.assertEqual(list(self.home.iterdir()), [])

    def test_install_check_idempotence_and_uninstall(self):
        config = self.home / '.codex/config.toml'
        config.parent.mkdir()
        config.write_text('unrelated')
        install.execute(SOURCE, self.home, 'apply')
        self.assertEqual(install.execute(SOURCE, self.home, 'check')['status'], 'verified')
        install.execute(SOURCE, self.home, 'apply')
        install.execute(SOURCE, self.home, 'uninstall')
        self.assertEqual(config.read_text(), 'unrelated')
        self.assertFalse((self.home/install.RECEIPT).exists())
        self.assertFalse((self.home/'.agents/skills/taeguk-test').exists())
        self.assertTrue((self.home/'.local/bin').is_dir())
        self.assertTrue((self.home/'.agents/skills').is_dir())

    def test_collision_preflight_preserves_all_files(self):
        path = self.home / '.local/bin/taeguk'
        path.parent.mkdir(parents=True)
        path.write_bytes(b'user binary')
        with self.assertRaises(ValueError):
            install.execute(SOURCE, self.home, 'apply')
        self.assertEqual(path.read_bytes(), b'user binary')
        self.assertFalse((self.home / '.codex/agents').exists())

    def test_modified_managed_file_blocks_update_and_removal(self):
        install.execute(SOURCE, self.home, 'apply')
        target = self.home / '.codex/agents/taeguk_test.toml'
        target.write_text('user edit')
        for mode in ('apply', 'check', 'uninstall'):
            with self.assertRaises(ValueError):
                install.execute(SOURCE, self.home, mode)
        self.assertEqual(target.read_text(), 'user edit')

    def test_symlink_parent_is_rejected(self):
        outside = self.home/'outside'
        outside.mkdir()
        (self.home/'.codex').symlink_to(outside, target_is_directory=True)
        with self.assertRaises(ValueError):
            install.execute(SOURCE, self.home, 'apply')
        self.assertEqual(list(outside.iterdir()), [])

    def test_write_failure_rolls_back(self):
        real = install.atomic_write
        def broken(path, data, mode):
            if path.name == 'SKILL.md':
                raise OSError('injected write failure')
            return real(path, data, mode)
        with patch.object(install, 'atomic_write', side_effect=broken):
            with self.assertRaises(OSError):
                install.execute(SOURCE, self.home, 'apply')
        self.assertFalse((self.home/'.codex/agents/taeguk_test.toml').exists())
        self.assertFalse((self.home/install.RECEIPT).exists())

    def test_uninstall_needs_only_the_receipt(self):
        install.execute(SOURCE, self.home, 'apply')
        with patch.object(install, 'payload', side_effect=ValueError('build the release CLI before installation')):
            install.execute(SOURCE, self.home, 'uninstall')
        self.assertFalse((self.home/'.local/bin/taeguk').exists())
        self.assertFalse((self.home/install.RECEIPT).exists())

    def test_uninstall_removes_files_dropped_from_current_bundle(self):
        install.execute(SOURCE, self.home, 'apply')
        self.bundle.pop('.agents/skills/taeguk-test/SKILL.md')
        with self.assertRaises(ValueError):
            install.execute(SOURCE, self.home, 'apply')
        install.execute(SOURCE, self.home, 'uninstall')
        self.assertFalse((self.home/'.agents/skills/taeguk-test').exists())

    def test_file_in_intermediate_path_fails_preflight(self):
        (self.home/'.agents').write_text('user file')
        with self.assertRaises(ValueError):
            install.execute(SOURCE, self.home, 'dry-run')
        self.assertFalse((self.home/'.codex').exists())

    def test_crlf_tolerance_applies_to_text_files_only(self):
        self.bundle['.codex/agents/taeguk_test.toml'] = (b'a\nb\n', 0o644)
        self.bundle['.local/bin/taeguk'] = (b'\x7fELF\nbin\n', 0o755)
        install.execute(SOURCE, self.home, 'apply')
        role = self.home/'.codex/agents/taeguk_test.toml'
        role.write_bytes(b'a\r\nb\r\n')
        self.assertEqual(install.execute(SOURCE, self.home, 'check')['status'], 'verified')
        cli = self.home/'.local/bin/taeguk'
        cli.write_bytes(b'\x7fELF\r\nbin\r\n')
        with self.assertRaises(ValueError):
            install.execute(SOURCE, self.home, 'check')

    def test_non_object_receipt_is_rejected(self):
        path = self.home/install.RECEIPT
        path.parent.mkdir(parents=True)
        path.write_text('[]')
        with self.assertRaises(ValueError):
            install.execute(SOURCE, self.home, 'apply')

    def test_unsafe_receipt_is_rejected(self):
        import json
        path=self.home/install.RECEIPT
        path.parent.mkdir(parents=True)
        path.write_text(json.dumps({'schema_version':1,'files':{'../outside':'0'*64}}))
        with self.assertRaises(ValueError):
            install.execute(SOURCE,self.home,'uninstall')


if __name__ == '__main__':
    unittest.main()
