#!/usr/bin/env python3
"""Temporary repository fixtures exercise actual skill drift, including failures."""

from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

import check_skills


class SkillDriftTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.canonical = self.root / ".agents" / "skills"
        self.bridge = self.root / ".claude" / "skills"
        self.bridge.parent.mkdir(parents=True)
        self.bridge.symlink_to("../.agents/skills", target_is_directory=True)
        for name in check_skills.ENTRYPOINTS:
            folder = self.canonical / name
            (folder / "agents").mkdir(parents=True)
            (folder / "references").mkdir()
            (folder / "scripts").mkdir()
            (folder / "SKILL.md").write_text(
                f"---\nname: {name}\ndescription: A focused Tabula workflow.\n---\n\n"
                "Read [the reference](references/guide.md).\n",
                encoding="utf-8",
            )
            (folder / "agents" / "openai.yaml").write_text(
                'interface:\n  display_name: "Tabula workflow"\n'
                '  short_description: "A focused workflow for this repository"\n'
                f'  default_prompt: "Use ${name} to handle this task."\n',
                encoding="utf-8",
            )
            (folder / "references" / "guide.md").write_text("# Reference\n", encoding="utf-8")
            (folder / "scripts" / "helper.py").write_text("print('helper')\n", encoding="utf-8")
        self.skill = self.canonical / "tabula-engineering" / "SKILL.md"
        self.ui = self.skill.parent / "agents" / "openai.yaml"

    def append(self, path, text):
        with path.open("a", encoding="utf-8") as handle:
            handle.write(text)

    def assert_error(self, text):
        errors = check_skills.check_repository(self.root)
        self.assertTrue(errors, "The broken fixture unexpectedly passed")
        self.assertTrue(any(text in error for error in errors), errors)

    def test_valid_canonical_skills_and_relative_bridge(self):
        self.assertEqual(check_skills.check_repository(self.root), [])

    def test_missing_frontmatter(self):
        self.skill.write_text("# No metadata\n", encoding="utf-8")
        self.assert_error("missing YAML frontmatter")

    def test_unclosed_frontmatter(self):
        self.skill.write_text("---\nname: tabula-engineering\n", encoding="utf-8")
        self.assert_error("unclosed YAML frontmatter")

    def test_yaml_syntax_error(self):
        self.skill.write_text("---\nname: [broken\n---\n", encoding="utf-8")
        self.assert_error("invalid YAML")

    def test_duplicate_yaml_keys(self):
        self.skill.write_text(
            "---\nname: tabula-engineering\nname: tabula-game-audit\ndescription: Duplicate.\n---\n",
            encoding="utf-8",
        )
        self.assert_error("duplicate mapping key")

    def test_missing_description(self):
        self.skill.write_text("---\nname: tabula-engineering\n---\n", encoding="utf-8")
        self.assert_error("description must be a nonempty string")

    def test_nonstring_description(self):
        self.skill.write_text("---\nname: tabula-engineering\ndescription: true\n---\n", encoding="utf-8")
        self.assert_error("description must be a nonempty string")

    def test_name_folder_mismatch(self):
        self.skill.write_text("---\nname: wrong-name\ndescription: A workflow.\n---\n", encoding="utf-8")
        self.assert_error("name must match folder")

    def test_missing_ui_metadata(self):
        self.ui.unlink()
        self.assert_error("cannot read file")

    def test_ui_metadata_types(self):
        self.ui.write_text("interface:\n  display_name: false\n  short_description: []\n  default_prompt: 42\n", encoding="utf-8")
        self.assert_error("interface.display_name must be a nonempty string")
        self.assert_error("interface.short_description must be a nonempty string")
        self.assert_error("interface.default_prompt must be a nonempty string")

    def test_wrong_ui_invocation(self):
        text = self.ui.read_text(encoding="utf-8")
        self.ui.write_text(text.replace("$tabula-engineering", "$tabula-game-audit"), encoding="utf-8")
        self.assert_error("default_prompt must mention $tabula-engineering")

    def test_prompt_does_not_accept_longer_name_prefix(self):
        text = self.ui.read_text(encoding="utf-8")
        self.ui.write_text(text.replace("$tabula-engineering", "$tabula-engineering-extra"), encoding="utf-8")
        self.assert_error("default_prompt must mention $tabula-engineering")

    def test_string_policy_boolean_is_rejected(self):
        self.append(self.ui, 'policy:\n  allow_implicit_invocation: "true"\n')
        self.assert_error("allow_implicit_invocation must be true")

    def test_duplicate_skill_name(self):
        other = self.canonical / "tabula-game-audit" / "SKILL.md"
        other.write_text(self.skill.read_text(encoding="utf-8"), encoding="utf-8")
        self.assert_error("duplicate skill name")

    def test_unexpected_canonical_definition(self):
        nested = self.skill.parent / "references" / "duplicate" / "SKILL.md"
        nested.parent.mkdir()
        nested.write_text(self.skill.read_text(encoding="utf-8"), encoding="utf-8")
        self.assert_error("unexpected canonical skill definition")

    def test_missing_required_entrypoint(self):
        shutil.rmtree(self.canonical / "tabula-game-audit")
        self.assert_error("missing required entrypoint")

    def test_broken_markdown_reference(self):
        self.append(self.skill, "Read [missing](references/missing.md).\n")
        self.assert_error("missing local resource: references/missing.md")

    def test_broken_plain_reference(self):
        self.append(self.skill, "Read `references/missing.md`.\n")
        self.assert_error("missing local resource: references/missing.md")

    def test_reference_link_is_relative_to_reference_file(self):
        reference = self.skill.parent / "references" / "guide.md"
        self.append(reference, "[Helper](../scripts/helper.py)\n")
        self.assertEqual(check_skills.check_repository(self.root), [])
        self.append(reference, "[Missing](../scripts/missing.py)\n")
        self.assert_error("missing local resource: ../scripts/missing.py")

    def test_stale_script_command_in_fenced_shell_example(self):
        self.append(self.skill, "```bash\npython3 .agents/skills/old-skill/scripts/helper.py --help\n```\n")
        self.assert_error("missing local resource: .agents/skills/old-skill/scripts/helper.py")

    def test_valid_script_command_in_fenced_shell_example(self):
        self.append(self.skill, "```bash\npython3 .agents/skills/tabula-engineering/scripts/helper.py crates/example/src\n```\n")
        self.assertEqual(check_skills.check_repository(self.root), [])

    def test_relative_script_command(self):
        self.append(self.skill, "```bash\npython3 scripts/missing.py\n```\n")
        self.assert_error("missing local resource: scripts/missing.py")

    def test_root_agents_and_skill_readme_links(self):
        (self.root / "AGENTS.md").write_text("[Missing](docs/missing.md)\n", encoding="utf-8")
        (self.canonical / "README.md").write_text("[Missing](../../missing.md)\n", encoding="utf-8")
        self.assert_error("AGENTS.md: missing local resource")
        self.assert_error("README.md: missing local resource")

    def test_urls_anchors_and_fenced_markdown_examples_are_ignored(self):
        self.append(
            self.skill,
            "[External](https://example.com/missing.md) [Section](#missing)\n"
            "[Email](mailto:test@example.com)\n```markdown\n[Illustration](example/missing.md)\n```\n"
            "```rust\nlet example = \"crates/example/src/file.rs\";\n```\n",
        )
        self.assertEqual(check_skills.check_repository(self.root), [])

    def test_independent_claude_copy_is_rejected(self):
        self.bridge.unlink()
        shutil.copytree(self.canonical, self.bridge)
        self.assert_error("independent definitions drift")

    def test_missing_claude_bridge_is_rejected(self):
        self.bridge.unlink()
        self.assert_error("must be a symlink")

    def test_bridge_to_another_directory_is_rejected(self):
        self.bridge.unlink()
        outside = self.root.parent / f"{self.root.name}-outside"
        outside.mkdir()
        self.addCleanup(shutil.rmtree, outside)
        self.bridge.symlink_to(f"../../{outside.name}", target_is_directory=True)
        self.assert_error("bridge must resolve to this repository")

    def test_absolute_bridge_is_rejected(self):
        self.bridge.unlink()
        self.bridge.symlink_to(self.canonical, target_is_directory=True)
        self.assert_error("bridge must use a relative symlink")

    def test_broken_bridge_is_rejected(self):
        self.bridge.unlink()
        self.bridge.symlink_to("../missing", target_is_directory=True)
        self.assert_error("bridge must resolve to this repository")

    def test_canonical_alias_is_rejected(self):
        (self.canonical / "alias").symlink_to("tabula-engineering", target_is_directory=True)
        self.assert_error("canonical definitions cannot be aliases")

    def test_resource_symlink_cannot_escape_checkout(self):
        outside = self.root.parent / f"{self.root.name}-outside.md"
        outside.write_text("outside\n", encoding="utf-8")
        self.addCleanup(outside.unlink)
        link = self.skill.parent / "references" / "outside.md"
        link.symlink_to(outside)
        self.append(self.skill, "[Escaped](references/outside.md)\n")
        self.assert_error("local resource escapes repository")

    def test_cli_nonzero_for_failure_and_zero_for_valid_repository(self):
        command = [sys.executable, str(Path(check_skills.__file__)), "--root", str(self.root)]
        result = subprocess.run(command, capture_output=True, text=True, check=False)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("2 canonical entrypoints", result.stdout)
        self.skill.unlink()
        result = subprocess.run(command, capture_output=True, text=True, check=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("missing required entrypoint", result.stderr)


if __name__ == "__main__":
    unittest.main()
