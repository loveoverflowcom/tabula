"""The ADR-0044 cap cannot silently replace the historical shell cap."""

import importlib.util
from pathlib import Path
import tempfile
import unittest


SPEC = importlib.util.spec_from_file_location(
    "loading_budgets", Path(__file__).with_name("check-loading-budgets.py")
)
BUDGETS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(BUDGETS)


class ShellBudgetTests(unittest.TestCase):
    def artifact(self, directory, size, marker=b""):
        root = Path(directory)
        (root / "index.html").write_text('<link href="/shell.wasm" rel="preload">')
        (root / "shell.wasm").write_bytes(marker + b"x" * (size - len(marker)))
        return root

    def test_historical_standard_cap_remains_exact(self):
        with tempfile.TemporaryDirectory() as directory:
            root = self.artifact(directory, 900_000)
            self.assertEqual(BUDGETS.shell_budget(root)["wasm_raw_limit"], 900_000)
            self.artifact(directory, 900_001)
            with self.assertRaisesRegex(AssertionError, "raw budget exceeded"):
                BUDGETS.shell_budget(root)

    def test_larger_cap_requires_expanded_marker_and_cannot_use_standard(self):
        with tempfile.TemporaryDirectory() as directory:
            root = self.artifact(directory, 1_100_000, b"tabula-shell-account-social-v1")
            self.assertEqual(BUDGETS.shell_budget(root, "account-social")["wasm_raw_limit"], 1_100_000)
            with self.assertRaisesRegex(AssertionError, "ambiguous compiled shell profile"):
                BUDGETS.shell_budget(root)
            self.artifact(directory, 1_100_001, b"tabula-shell-account-social-v1")
            with self.assertRaisesRegex(AssertionError, "raw budget exceeded"):
                BUDGETS.shell_budget(root, "account-social")
            self.artifact(directory, 1_000_000)
            with self.assertRaisesRegex(AssertionError, "compiled shell profile does not match"):
                BUDGETS.shell_budget(root, "account-social")

    def test_conflicting_markers_do_not_authorize_larger_cap(self):
        with tempfile.TemporaryDirectory() as directory:
            root = self.artifact(directory, 1_000_000, b"tabula-shell-standard-v1 tabula-shell-account-social-v1")
            with self.assertRaisesRegex(AssertionError, "ambiguous compiled shell profile"):
                BUDGETS.shell_budget(root, "account-social")


if __name__ == "__main__":
    unittest.main()
