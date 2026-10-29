import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

SCRIPT = Path(__file__).resolve().parents[1]/'scripts/check_worker_scope.py'

RUST = '''fn add(a: i32, b: i32) -> i32 {
    a + b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds() {
        assert_eq!(add(1, 2), 3);
    }
}
'''


class WorkerScopeTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.repo = Path(self.tmp.name)
        self.git('init', '-q', '-b', 'main')
        self.write('README.md', 'base\n')
        self.write('tests/test_a.py', 'import unittest\n')
        self.write('scripts/tool.py', 'print(1)\n')
        self.write('tools/taeguk-cli/src/main.rs', RUST)
        self.write('apps/taeguk/lib/a.ex', 'defmodule A do\nend\n')
        self.base = self.commit('base')
        self.git('checkout', '-q', '-b', 'work')

    def git(self, *args):
        env = {**os.environ, 'GIT_AUTHOR_NAME': 't', 'GIT_AUTHOR_EMAIL': 't@example.invalid',
               'GIT_COMMITTER_NAME': 't', 'GIT_COMMITTER_EMAIL': 't@example.invalid'}
        return subprocess.run(['git', *args], cwd=self.repo, env=env, check=True, capture_output=True, text=True).stdout

    def write(self, path, text, mode=None):
        target = self.repo/path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(text if isinstance(text, bytes) else text.encode())
        if mode:
            target.chmod(mode)

    def commit(self, message):
        self.git('add', '-A')
        self.git('commit', '-q', '-m', message)
        return self.git('rev-parse', 'HEAD').strip()

    def run_check(self, worker, base=None):
        out = subprocess.run([sys.executable, str(SCRIPT), worker, '--base', base or self.base, '--head', 'HEAD'],
                             cwd=self.repo, capture_output=True, text=True)
        return out.returncode, json.loads(out.stdout)

    def assert_rejected(self, worker, path, reason):
        code, result = self.run_check(worker)
        self.assertEqual((code, result['status']), (1, 'FAIL'), result)
        self.assertIn({'path': path, 'reason': reason}, result['rejected'])

    def test_test_gap_accepts_new_and_edited_tests(self):
        self.write('tests/test_a.py', 'import unittest\n\nclass T(unittest.TestCase):\n    pass\n')
        self.write('apps/taeguk/test/b_test.exs', 'ExUnit.start()\n')
        self.commit('tests')
        code, result = self.run_check('test-gap')
        self.assertEqual((code, result['status'], result['changed']), (0, 'PASS', 2))

    def test_test_gap_rejects_product_code(self):
        self.write('scripts/tool.py', 'print(2)\n')
        self.commit('product')
        self.assert_rejected('test-gap', 'scripts/tool.py', 'not a test path')

    def test_test_gap_accepts_edit_inside_trailing_rust_test_module(self):
        self.write('tools/taeguk-cli/src/main.rs', RUST.replace('    }\n}\n', '    }\n\n    #[test]\n    fn braces_in_strings() {\n        assert_eq!("}", "}");\n        let _ = \'{\';\n    }\n}\n'))
        self.commit('rust test')
        code, result = self.run_check('test-gap')
        self.assertEqual((code, result['status']), (0, 'PASS'), result)

    def test_test_gap_rejects_rust_edit_before_test_module(self):
        self.write('tools/taeguk-cli/src/main.rs', RUST.replace('a + b', 'a - b'))
        self.commit('product edit')
        self.assert_rejected('test-gap', 'tools/taeguk-cli/src/main.rs', 'change outside the trailing #[cfg(test)] module')

    def test_test_gap_rejects_rust_code_after_test_module(self):
        self.write('tools/taeguk-cli/src/main.rs', RUST + '\nfn extra() {}\n')
        self.commit('code after tests')
        self.assert_rejected('test-gap', 'tools/taeguk-cli/src/main.rs', 'change outside the trailing #[cfg(test)] module')

    def test_test_gap_rejects_second_test_module(self):
        self.write('tools/taeguk-cli/src/main.rs', RUST.replace('fn add', '#[cfg(test)]\nfn helper() {}\n\nfn add'))
        self.commit('second cfg')
        self.assert_rejected('test-gap', 'tools/taeguk-cli/src/main.rs', 'change outside the trailing #[cfg(test)] module')

    def test_test_gap_rejects_cfg_test_item_that_is_not_a_module(self):
        source = 'fn add() {}\n\n#[cfg(test)]\nfn helper() {\n    let _ = 1;\n}\n'
        self.write('tools/taeguk-cli/src/lib.rs', source)
        self.base = self.commit('cfg fn')
        self.write('tools/taeguk-cli/src/lib.rs', source.replace('1;', '2;'))
        self.commit('edit cfg fn')
        self.assert_rejected('test-gap', 'tools/taeguk-cli/src/lib.rs', 'change outside the trailing #[cfg(test)] module')

    def test_test_gap_rejects_code_after_module_on_closing_line(self):
        self.write('tools/taeguk-cli/src/main.rs', RUST[:-2] + '} impl Drop for X { fn drop(&mut self) {\n    std::process::exit(0); } }\n')
        self.commit('code on closing line')
        self.assert_rejected('test-gap', 'tools/taeguk-cli/src/main.rs', 'change outside the trailing #[cfg(test)] module')

    def test_test_gap_braces_in_comments_do_not_move_the_boundary(self):
        self.write('tools/taeguk-cli/src/main.rs', RUST.replace('    }\n}\n', '    }\n    /* { */\n}\nfn outside() {}\n/* } */\n'))
        self.commit('comment braces')
        self.assert_rejected('test-gap', 'tools/taeguk-cli/src/main.rs', 'change outside the trailing #[cfg(test)] module')

    def test_test_gap_accepts_multiline_and_raw_strings_in_tests(self):
        self.write('tools/taeguk-cli/src/main.rs', RUST.replace('    }\n}\n', '    }\n\n    #[test]\n    fn text() {\n        let a = "line one {\n}";\n        let b = r#"raw "}" {"#;\n        let c: &\'static str = "x";\n        assert!(a.len() + b.len() + c.len() > 0); // }\n        /* } /* nested } */ */\n    }\n}\n'))
        self.commit('strings')
        code, result = self.run_check('test-gap')
        self.assertEqual((code, result['status']), (0, 'PASS'), result)

    def test_raw_c_string_does_not_hide_code_after_module(self):
        self.write('tools/taeguk-cli/src/main.rs', RUST[:-2] + '    const C: &core::ffi::CStr = cr#"" /*"#;\n}\npub fn injected() {}\n// */ }\n')
        self.commit('raw c string')
        self.assert_rejected('test-gap', 'tools/taeguk-cli/src/main.rs', 'change outside the trailing #[cfg(test)] module')

    def test_modules_shadowing_the_standard_library_are_rejected(self):
        for path in ('scripts/json.py', 'tests/unittest/__init__.py', 'scripts/sitecustomize.py', 'tests/Os.py'):
            with self.subTest(path=path):
                self.git('reset', '-q', '--hard', self.base)
                self.write(path, 'import os\nos._exit(0)\n')
                self.commit(path)
                for worker in ('test-gap', 'daily-cleanup'):
                    self.assert_rejected(worker, path, 'shadows a standard library module')

    def test_isolated_checker_ignores_modules_added_by_the_pr(self):
        self.write('scripts/json.py', 'import os\nprint(\'{"status": "PASS"}\')\nos._exit(0)\n')
        self.write('AGENTS.md', 'changed\n')
        self.commit('shadow json')
        trusted = Path(self.tmp.name + '-trusted')
        trusted.mkdir()
        self.addCleanup(lambda: __import__('shutil').rmtree(trusted))
        (trusted/'check_worker_scope.py').write_text(SCRIPT.read_text())
        out = subprocess.run([sys.executable, '-I', str(trusted/'check_worker_scope.py'), 'daily-cleanup',
                              '--base', self.base, '--head', 'HEAD'], cwd=self.repo, capture_output=True, text=True)
        self.assertEqual(out.returncode, 1, out.stdout)
        self.assertIn({'path': 'AGENTS.md', 'reason': 'protected path'}, json.loads(out.stdout)['rejected'])

    def test_test_gap_rejects_deleting_tests(self):
        (self.repo/'tests/test_a.py').unlink()
        self.commit('delete')
        self.assert_rejected('test-gap', 'tests/test_a.py', 'test-gap does not delete files')

    def test_protected_paths_are_rejected_for_both_workers_in_any_case(self):
        for path in ('AGENTS.md', 'docs/Agents.md', '.GitHub/workflows/x.yml', 'codex/skills/x/SKILL.md',
                     'Docs/Rules/registry.toml', 'tools/taeguk-cli/Cargo.lock', 'tests/.gitattributes',
                     'scripts/check_worker_scope.py', 'docs/repository-operations.md',
                     'tests/.agents/skills/x/SKILL.md', 'apps/taeguk/test/.Claude/agents/x.md'):
            with self.subTest(path=path):
                self.git('reset', '-q', '--hard', self.base)
                self.write(path, 'x\n')
                self.commit(path)
                for worker in ('test-gap', 'daily-cleanup'):
                    self.assert_rejected(worker, path, 'protected path')

    def test_symlink_binary_and_mode_changes_are_rejected(self):
        os.symlink('../scripts/tool.py', self.repo/'tests/link.py')
        self.write('tests/data.bin', b'\x00\x01')
        self.write('tests/run.py', 'print(1)\n', 0o755)
        self.commit('odd files')
        code, result = self.run_check('test-gap')
        reasons = {r['path']: r['reason'] for r in result['rejected']}
        self.assertEqual(code, 1)
        self.assertEqual(reasons, {'tests/link.py': 'not a regular file (mode 120000)',
                                   'tests/data.bin': 'binary content',
                                   'tests/run.py': 'new file is not mode 100644'})

    def test_mode_change_and_non_ascii_path_are_rejected(self):
        (self.repo/'tests/test_a.py').chmod(0o755)
        self.write('tests/teſt.py', 'x\n')
        self.commit('mode and name')
        code, result = self.run_check('daily-cleanup')
        reasons = {r['reason'] for r in result['rejected']}
        self.assertEqual(code, 1)
        self.assertEqual(reasons, {'file mode change', 'non-ASCII or control character in path'})

    def test_daily_cleanup_accepts_cleanup_paths_and_deletions(self):
        self.write('apps/taeguk/lib/a.ex', 'defmodule A do\n  # fixed comment\nend\n')
        self.write('docs/x.md', '# x\n')
        (self.repo/'scripts/tool.py').unlink()
        self.commit('cleanup')
        code, result = self.run_check('daily-cleanup')
        self.assertEqual((code, result['status'], result['changed']), (0, 'PASS', 3), result)

    def test_daily_cleanup_rejects_paths_outside_cleanup(self):
        self.write('apps/taeguk/priv/repo/migrations/1.exs', 'x\n')
        self.commit('migration')
        self.assert_rejected('daily-cleanup', 'apps/taeguk/priv/repo/migrations/1.exs', 'not a cleanup path')

    def test_range_starts_at_merge_base(self):
        self.write('tests/test_a.py', 'import unittest  # more\n')
        self.commit('test')
        self.git('checkout', '-q', 'main')
        self.write('AGENTS.md', 'moved main\n')
        main = self.commit('main moved')
        self.git('checkout', '-q', 'work')
        code, result = self.run_check('test-gap', base=main)
        self.assertEqual((code, result['status'], result['changed']), (0, 'PASS', 1), result)

    def test_unknown_base_is_unknown_not_pass(self):
        code, result = self.run_check('test-gap', base='no-such-ref')
        self.assertEqual((code, result['status']), (2, 'UNKNOWN'))


if __name__ == '__main__':
    unittest.main()
