"""Bounded policy and actual-index regression tests for repository hygiene."""
import importlib.util
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('hygiene', ROOT / 'tools/check-repository-hygiene.py')
hygiene = importlib.util.module_from_spec(spec)
spec.loader.exec_module(hygiene)


class RepositoryHygieneTest(unittest.TestCase):
    def test_generated_document_evidence_is_rejected(self):
        for folder in ['docs/verification/issue-999', 'docs/ui/new-design']:
            for name in ['preview.png', 'preview.jpg', 'design.svg', 'index.html', 'style.css',
                         'preview.mjs', 'export.zip', 'tests.log', 'coverage.json', 'tests.xml', 'results.gz']:
                with self.subTest(folder=folder, name=name):
                    self.assertTrue(hygiene.forbidden(f'{folder}/{name}'))

    def test_markdown_and_generated_token_contract_are_kept(self):
        for name in ['docs/verification/issue-999/README.md', 'docs/ui/screens/01-library.md',
                     'docs/ui/issue-82-game-feel/PROMPTS.md', 'docs/ui/tokens.json']:
            self.assertFalse(hygiene.forbidden(name), name)

    def test_source_runtime_mobile_and_fixtures_are_unrestricted(self):
        for name in ['games/chess/assets/pieces.png', 'games/werewolf/assets/source/design-provenance.json',
                     'games/werewolf/assets/budgets.json', 'assets/brand/logo.svg',
                     'apps/web/public/brand/app-icon-180.png', 'apps/mobile/android/src/main/res/icon.png',
                     'apps/mobile/ios/Tabula/Assets.xcassets/AppIcon.appiconset/icon.png',
                     'apps/mobile/gradle/wrapper/gradle-wrapper.jar',
                     'tests/online-match/chess_board_layout_fixture.csv']:
            self.assertFalse(hygiene.forbidden(name), name)

    def test_nearby_directory_names_are_not_restricted(self):
        for name in ['docs/verification-tools/config.json', 'docs/ui-components/source.svg', 'tools/verification/run.py']:
            self.assertFalse(hygiene.forbidden(name), name)

    def test_index_rejects_force_added_ignored_evidence(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            subprocess.run(['git', 'init', '-q', str(root)], check=True)
            (root / '.gitignore').write_text('/docs/verification/**\n')
            evidence = root / 'docs/verification/check/receipt.json'
            evidence.parent.mkdir(parents=True)
            evidence.write_text('{}\n')
            subprocess.run(['git', 'add', '-f', str(evidence)], cwd=root, check=True)
            self.assertEqual(hygiene.check(root), ['docs/verification/check/receipt.json'])

    def test_untracked_local_output_is_not_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            subprocess.run(['git', 'init', '-q', str(root)], check=True)
            evidence = root / 'docs/verification/check/receipt.json'
            evidence.parent.mkdir(parents=True)
            evidence.write_text('{}\n')
            self.assertEqual(hygiene.check(root), [])


if __name__ == '__main__':
    unittest.main()
