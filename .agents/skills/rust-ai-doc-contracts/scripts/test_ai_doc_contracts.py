#!/usr/bin/env python3
"""Tests for ai_doc_contracts.py."""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

import ai_doc_contracts as contracts


VALID_SOURCE = """\
/// Applies a legal command transactionally.
///
/// @ai.role domain-transition
/// @ai.domain game.rules
/// @ai.pure true
/// @ai.invariant rejected-input-preserves-state
/// @ai.law deterministic-transition
/// @ai.evidence tests::apply_properties
/// @ai.related crate::State
pub fn apply(state: &State, command: Command) -> Outcome {
    todo!()
}
"""


class SourceContractTests(unittest.TestCase):
    def test_valid_source_builds_typed_edges(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "rules.rs"
            path.write_text(VALID_SOURCE, encoding="utf-8")

            items, issues = contracts.source_contracts([path])
            self.assertEqual([], issues)
            self.assertEqual(1, len(items))

            graph = contracts.build_graph(items)
            edge_types = {edge["type"] for edge in graph["edges"]}
            self.assertTrue(
                {"role", "domain", "property", "preserves", "satisfies", "evidenced_by"}
                <= edge_types
            )

    def test_law_without_evidence_and_unknown_tag_fail(self) -> None:
        source = """\
/// @ai.role reducer
/// @ai.law deterministic-transition
/// @ai.important very
pub fn reduce() {}
"""
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "rules.rs"
            path.write_text(source, encoding="utf-8")

            _, issues = contracts.source_contracts([path])
            messages = {issue.message for issue in issues}
            self.assertIn("unknown tag @ai.important", messages)
            self.assertIn(
                "items with @ai.invariant or @ai.law require @ai.evidence",
                messages,
            )

    def test_malformed_values_are_rejected(self) -> None:
        tags = [
            contracts.Tag("role", "Domain Transition", 1),
            contracts.Tag("pure", "yes", 2),
            contracts.Tag("related", "tests/reducer.rs", 3),
        ]
        issues = contracts.validate_tags(tags, "fixture")
        self.assertEqual(3, sum(issue.severity == "error" for issue in issues))

    def test_missing_source_path_fails_instead_of_reporting_green(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            missing = Path(directory) / "misspelled.rs"
            with self.assertRaises(FileNotFoundError):
                contracts.source_contracts([missing])


class RustdocContractTests(unittest.TestCase):
    def test_rustdoc_metadata_and_links_are_preserved(self) -> None:
        payload = {
            "index": {
                "0:1:0": {
                    "name": "reconcile_nodes",
                    "docs": (
                        "Reconciles nodes.\n\n"
                        "@ai.role domain-transition\n"
                        "@ai.law preserves-unrelated-nodes\n"
                        "@ai.evidence tests::reconcile_properties"
                    ),
                    "inner": {"function": {}},
                    "span": {"filename": "src/lib.rs", "begin": [10, 1], "end": [20, 2]},
                    "visibility": "public",
                    "attrs": ["#[must_use]"],
                    "links": {"Document": "0:2:0"},
                }
            }
        }

        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "crate.json"
            path.write_text(json.dumps(payload), encoding="utf-8")

            items, issues = contracts.rustdoc_contracts(path)
            self.assertEqual([], issues)
            graph = contracts.build_graph(items)
            item = next(node for node in graph["nodes"] if node["type"] == "rust_item")
            self.assertEqual("0:1:0", item["rustdoc_id"])
            self.assertEqual(["#[must_use]"], item["attributes"])
            self.assertEqual({"Document": "0:2:0"}, item["links"])
            self.assertIn(
                "rustdoc_link",
                {edge["type"] for edge in graph["edges"]},
            )

    def test_dot_output_escapes_labels(self) -> None:
        graph = {
            "nodes": [{"id": 'node:"x"', "name": "line\nname", "type": "symbol"}],
            "edges": [],
        }
        rendered = contracts.graph_as_dot(graph)
        self.assertIn(r'node:\"x\"', rendered)
        self.assertIn(r"line\nname", rendered)


if __name__ == "__main__":
    unittest.main()
