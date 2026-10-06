"""Runs synthetic Node/vm observer contracts; no actual-browser evidence is implied."""

from pathlib import Path
import re
import shutil
import subprocess
import unittest


class ActualFetchObserverJavascriptTests(unittest.TestCase):
    def test_node_observer_contracts_run_nonempty_and_without_skips(self):
        node = shutil.which("node")
        self.assertIsNotNone(node, "Node with built-in Fetch and node:test is required for observer helper tests")
        suite = Path(__file__).with_name("test_actual_fetch_observer.cjs")
        result = subprocess.run(
            [node, "--test", "--test-reporter=tap", str(suite)],
            cwd=suite.parent,
            capture_output=True,
            text=True,
            timeout=60,
            check=False,
        )
        output = result.stdout + result.stderr
        self.assertEqual(result.returncode, 0, output)
        counts = dict(re.findall(r"^# (tests|pass|fail|cancelled|skipped) (\d+)\s*$", output, re.MULTILINE))
        self.assertGreater(int(counts.get("tests", "0")), 0, output)
        self.assertEqual(counts.get("pass"), counts["tests"], output)
        for outcome in ("fail", "cancelled", "skipped"):
            self.assertEqual(counts.get(outcome), "0", output)


if __name__ == "__main__":
    unittest.main()
