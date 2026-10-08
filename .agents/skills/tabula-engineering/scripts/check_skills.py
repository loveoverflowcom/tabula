#!/usr/bin/env python3
"""Check Tabula's canonical skill metadata, local resources, and Claude bridge.

Requires Python 3.10+ and PyYAML (also used by the skill-creator validators).
This is a structural drift check, not a Markdown renderer or a behavior audit.
Inline Markdown links are checked outside fenced examples; plain script/resource
paths are checked even in examples when they refer to the actual skills tree.
External URLs, anchors, and illustrative paths outside that tree are ignored.
"""

from __future__ import annotations

import argparse
from pathlib import Path
import re
import sys
from urllib.parse import unquote, urlsplit

import yaml


ENTRYPOINTS = (
    "tabula-engineering",
    "tabula-game-audit",
    "tabula-code-review",
    "tabula-cmp-engineering",
)
NAME = re.compile(r"[a-z0-9]+(?:-[a-z0-9]+)*\Z")
INLINE_LINK = re.compile(r"!?\[[^\]\n]*\]\(\s*(<[^>\n]+>|[^\s)]+)(?:\s+[\"'][^\n]*[\"'])?\s*\)")
RESOURCE_PATH = re.compile(
    r"(?<![\w./-])(?:\./|\.\./)*(?:\.agents/skills/|\.claude/skills/|scripts/|references/)"
    r"[\w.-]+(?:/[\w.-]+)*"
)
FENCE = re.compile(r"^\s{0,3}(`{3,}|~{3,})")


class UniqueKeyLoader(yaml.SafeLoader):
    """Use PyYAML's safe parser, but reject silently overwritten mapping keys."""

    def construct_mapping(self, node, deep=False):
        self.flatten_mapping(node)
        mapping = {}
        for key_node, value_node in node.value:
            key = self.construct_object(key_node, deep=deep)
            try:
                duplicate = key in mapping
            except TypeError as error:
                raise yaml.constructor.ConstructorError(
                    None, None, "unhashable mapping key", key_node.start_mark
                ) from error
            if duplicate:
                raise yaml.constructor.ConstructorError(
                    None, None, f"duplicate mapping key: {key!r}", key_node.start_mark
                )
            mapping[key] = self.construct_object(value_node, deep=deep)
        return mapping


def _label(path: Path, root: Path) -> str:
    try:
        return path.relative_to(root).as_posix()
    except ValueError:
        return str(path)


def _read(path: Path, root: Path, errors: list[str]) -> str | None:
    try:
        return path.read_text(encoding="utf-8")
    except (OSError, UnicodeError) as error:
        errors.append(f"{_label(path, root)}: cannot read file: {error}")
        return None


def _yaml_mapping(text: str, path: Path, root: Path, errors: list[str]) -> dict | None:
    try:
        value = yaml.load(text, Loader=UniqueKeyLoader)
    except yaml.YAMLError as error:
        errors.append(f"{_label(path, root)}: invalid YAML: {error}")
        return None
    if not isinstance(value, dict):
        errors.append(f"{_label(path, root)}: YAML metadata must be a mapping")
        return None
    return value


def _frontmatter(text: str, path: Path, root: Path, errors: list[str]) -> dict | None:
    lines = text.splitlines()
    if not lines or lines[0] != "---":
        errors.append(f"{_label(path, root)}: missing YAML frontmatter")
        return None
    try:
        end = lines.index("---", 1)
    except ValueError:
        errors.append(f"{_label(path, root)}: unclosed YAML frontmatter")
        return None
    return _yaml_mapping("\n".join(lines[1:end]), path, root, errors)


def _nonempty_string(value: object) -> bool:
    return isinstance(value, str) and bool(value.strip())


def _check_metadata(skill: Path, root: Path, errors: list[str]) -> str | None:
    text = _read(skill, root, errors)
    metadata = _frontmatter(text, skill, root, errors) if text is not None else None
    name = metadata.get("name") if metadata is not None else None
    if metadata is not None:
        for key in ("name", "description"):
            if not _nonempty_string(metadata.get(key)):
                errors.append(f"{_label(skill, root)}: {key} must be a nonempty string")
        if isinstance(name, str):
            if not NAME.fullmatch(name) or len(name) > 64:
                errors.append(f"{_label(skill, root)}: invalid skill name {name!r}")
            if name != skill.parent.name:
                errors.append(f"{_label(skill, root)}: name must match folder {skill.parent.name!r}")
        for key in ("license", "compatibility"):
            if key in metadata and not _nonempty_string(metadata[key]):
                errors.append(f"{_label(skill, root)}: {key} must be a nonempty string")
        if "metadata" in metadata and not isinstance(metadata["metadata"], dict):
            errors.append(f"{_label(skill, root)}: metadata must be a mapping")

    ui_path = skill.parent / "agents" / "openai.yaml"
    ui_text = _read(ui_path, root, errors)
    ui = _yaml_mapping(ui_text, ui_path, root, errors) if ui_text is not None else None
    if ui is not None:
        interface = ui.get("interface")
        if not isinstance(interface, dict):
            errors.append(f"{_label(ui_path, root)}: interface must be a mapping")
        else:
            for key in ("display_name", "short_description", "default_prompt"):
                if not _nonempty_string(interface.get(key)):
                    errors.append(f"{_label(ui_path, root)}: interface.{key} must be a nonempty string")
            short = interface.get("short_description")
            if isinstance(short, str) and not 25 <= len(short) <= 64:
                errors.append(f"{_label(ui_path, root)}: short_description must be 25–64 characters")
            prompt = interface.get("default_prompt")
            invocation = rf"\${re.escape(skill.parent.name)}(?![\w-])"
            if isinstance(prompt, str) and not re.search(invocation, prompt):
                errors.append(f"{_label(ui_path, root)}: default_prompt must mention ${skill.parent.name}")
            for key in ("icon_small", "icon_large"):
                if key in interface:
                    if not _nonempty_string(interface[key]):
                        errors.append(f"{_label(ui_path, root)}: interface.{key} must be a nonempty string")
                    else:
                        _check_target(interface[key], ui_path, root, errors, base=skill.parent)
            if "brand_color" in interface and (
                not isinstance(interface["brand_color"], str)
                or not re.fullmatch(r"#[0-9a-fA-F]{6}", interface["brand_color"])
            ):
                errors.append(f"{_label(ui_path, root)}: brand_color must be a six-digit hex string")
        if "policy" in ui:
            policy = ui["policy"]
            if not isinstance(policy, dict):
                errors.append(f"{_label(ui_path, root)}: policy must be a mapping")
            elif "allow_implicit_invocation" in policy and policy["allow_implicit_invocation"] is not True:
                errors.append(f"{_label(ui_path, root)}: allow_implicit_invocation must be true")
    return name if isinstance(name, str) else None


def _check_target(
    target: str, source: Path, root: Path, errors: list[str], *, base: Path | None = None
) -> None:
    target = target.strip("<>")
    parsed = urlsplit(target)
    if parsed.scheme or parsed.netloc or not parsed.path:
        return
    path = (base or source.parent) / unquote(parsed.path)
    try:
        resolved = path.resolve()
        if not resolved.is_relative_to(root):
            errors.append(f"{_label(source, root)}: local resource escapes repository: {target}")
        elif not path.exists():
            errors.append(f"{_label(source, root)}: missing local resource: {target}")
    except (OSError, RuntimeError) as error:
        errors.append(f"{_label(source, root)}: invalid local resource {target}: {error}")


def _check_links(path: Path, root: Path, errors: list[str]) -> None:
    text = _read(path, root, errors)
    if text is None:
        return
    fence: str | None = None
    seen: set[str] = set()
    for line in text.splitlines():
        marker = FENCE.match(line)
        if marker:
            if fence is None:
                fence = marker[1]
            elif marker[1][0] == fence[0] and len(marker[1]) >= len(fence):
                fence = None
            continue
        if fence is None:
            for match in INLINE_LINK.finditer(line):
                target = match[1]
                if target not in seen:
                    _check_target(target, path, root, errors)
                    seen.add(target)
        for match in RESOURCE_PATH.finditer(line):
            target = match[0]
            # Generic directory names ("references/") are not file references.
            if line[match.end():].startswith("/") or target in seen:
                continue
            repository_path = target.startswith((".agents/skills/", ".claude/skills/", "./.agents/skills/", "./.claude/skills/"))
            _check_target(target, path, root, errors, base=root if repository_path else None)
            seen.add(target)


def check_repository(root: Path | str) -> list[str]:
    """Return actionable structural errors; an empty list means these checks passed."""
    root = Path(root).resolve()
    canonical = root / ".agents" / "skills"
    bridge = root / ".claude" / "skills"
    errors: list[str] = []
    if canonical.is_symlink() or not canonical.is_dir():
        errors.append(".agents/skills: canonical skills must be a real directory")
    if not bridge.is_symlink():
        errors.append(".claude/skills: must be a symlink to ../.agents/skills; independent definitions drift")
    else:
        try:
            if bridge.resolve() != canonical.resolve() or not bridge.is_dir():
                errors.append(".claude/skills: bridge must resolve to this repository's .agents/skills")
            if Path(bridge.readlink()).is_absolute():
                errors.append(".claude/skills: bridge must use a relative symlink")
        except (OSError, RuntimeError) as error:
            errors.append(f".claude/skills: invalid bridge: {error}")

    names: dict[str, Path] = {}
    definitions = sorted(canonical.rglob("SKILL.md")) if canonical.is_dir() else []
    for name in ENTRYPOINTS:
        if not (canonical / name / "SKILL.md").is_file():
            errors.append(f".agents/skills/{name}/SKILL.md: missing required entrypoint")
    if canonical.is_dir():
        for child in canonical.iterdir():
            if child.is_symlink():
                errors.append(f"{_label(child, root)}: canonical definitions cannot be aliases")
    for skill in definitions:
        if skill.parent.parent != canonical or skill.parent.name not in ENTRYPOINTS:
            errors.append(f"{_label(skill, root)}: unexpected canonical skill definition")
        if skill.is_symlink():
            errors.append(f"{_label(skill, root)}: canonical definition cannot be a symlink")
        name = _check_metadata(skill, root, errors)
        if name is not None:
            if name in names:
                errors.append(f"{_label(skill, root)}: duplicate skill name {name!r}; also in {_label(names[name], root)}")
            names[name] = skill

    markdown = set(canonical.rglob("*.md")) if canonical.is_dir() else set()
    for name in ("AGENTS.md", "CLAUDE.md"):
        if (root / name).is_file():
            markdown.add(root / name)
    for path in sorted(markdown):
        _check_links(path, root, errors)
    return errors


def _default_root() -> Path:
    for parent in Path(__file__).resolve().parents:
        if (parent / "AGENTS.md").is_file() and (parent / ".agents" / "skills").is_dir():
            return parent
    raise SystemExit("Cannot discover repository root; pass --root PATH")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, help="Repository root (default: discover from script location)")
    args = parser.parse_args()
    errors = check_repository(args.root or _default_root())
    if errors:
        for error in errors:
            print(f"ERROR: {error}", file=sys.stderr)
        print(f"Skill drift check failed: {len(errors)} error(s)", file=sys.stderr)
        return 1
    print(f"Skill drift check passed: {len(ENTRYPOINTS)} canonical entrypoints, local resources, and Claude bridge")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
