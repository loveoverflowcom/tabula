#!/usr/bin/env python3
"""Validate and index structured @ai.* contracts in Rust documentation.

This tool deliberately uses only Python's standard library. Source mode is a conservative
navigation index, not a Rust parser. Rustdoc JSON mode preserves compiler-produced metadata.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Iterable, Sequence


SCHEMA_VERSION = 1
SINGLETON_KEYS = {"role", "domain", "pure"}
REPEATABLE_KEYS = {
    "invariant",
    "law",
    "requires",
    "ensures",
    "evidence",
    "read-first",
    "related",
}
KNOWN_KEYS = SINGLETON_KEYS | REPEATABLE_KEYS

TAG_RE = re.compile(r"^@ai\.([a-z][a-z0-9-]*)(?:\s+(.+?))?\s*$")
DOC_RE = re.compile(r"^\s*(?P<prefix>///|//!) ?(?P<body>.*)$")
KEBAB_RE = re.compile(r"^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$")
DOMAIN_RE = re.compile(
    r"^[a-z][a-z0-9]*(?:-[a-z0-9]+)*(?:\.[a-z][a-z0-9]*(?:-[a-z0-9]+)*)*$"
)
RUST_PATH_RE = re.compile(
    r"^(?:(?:r#)?[A-Za-z_][A-Za-z0-9_]*)(?:::(?:r#)?[A-Za-z_][A-Za-z0-9_]*)*$"
)

ITEM_PATTERNS: tuple[tuple[str, re.Pattern[str]], ...] = (
    ("function", re.compile(r"\bfn\s+([A-Za-z_][A-Za-z0-9_]*)\b")),
    ("struct", re.compile(r"\bstruct\s+([A-Za-z_][A-Za-z0-9_]*)\b")),
    ("enum", re.compile(r"\benum\s+([A-Za-z_][A-Za-z0-9_]*)\b")),
    ("trait", re.compile(r"\btrait\s+([A-Za-z_][A-Za-z0-9_]*)\b")),
    ("type", re.compile(r"\btype\s+([A-Za-z_][A-Za-z0-9_]*)\b")),
    ("module", re.compile(r"\bmod\s+([A-Za-z_][A-Za-z0-9_]*)\b")),
    ("constant", re.compile(r"\bconst\s+([A-Za-z_][A-Za-z0-9_]*)\b")),
    ("static", re.compile(r"\bstatic\s+(?:mut\s+)?([A-Za-z_][A-Za-z0-9_]*)\b")),
    ("union", re.compile(r"\bunion\s+([A-Za-z_][A-Za-z0-9_]*)\b")),
    ("macro", re.compile(r"\bmacro_rules!\s*([A-Za-z_][A-Za-z0-9_]*)\b")),
)

EDGE_MAP = {
    "role": ("role", "role"),
    "domain": ("domain", "domain"),
    "pure": ("property", "property"),
    "invariant": ("preserves", "invariant"),
    "law": ("satisfies", "law"),
    "requires": ("requires", "predicate"),
    "ensures": ("ensures", "predicate"),
    "evidence": ("evidenced_by", "symbol"),
    "read-first": ("read_first", "symbol"),
    "related": ("related_to", "symbol"),
}


@dataclass(frozen=True)
class Tag:
    """One parsed contract tag."""

    key: str
    value: str
    line: int | None = None


@dataclass(frozen=True)
class Issue:
    """A schema error or warning with a stable display location."""

    severity: str
    location: str
    message: str


@dataclass
class Contract:
    """An annotated Rust item from source or rustdoc JSON."""

    item_id: str
    name: str
    kind: str
    docs: str
    tags: list[Tag]
    span: Any = None
    visibility: Any = None
    attributes: Any = field(default_factory=list)
    links: dict[str, Any] = field(default_factory=dict)
    source: str = "source"
    source_path: str | None = None
    line: int | None = None
    rustdoc_id: str | None = None


def parse_tags(lines: Iterable[tuple[int | None, str]], location: str) -> tuple[list[Tag], list[Issue]]:
    """Parse tags from already-unwrapped doc lines."""

    tags: list[Tag] = []
    issues: list[Issue] = []

    for line_number, raw_body in lines:
        body = raw_body.strip()
        if "@ai." not in body:
            continue
        match = TAG_RE.fullmatch(body)
        line_location = f"{location}:{line_number}" if line_number is not None else location
        if match is None:
            issues.append(Issue("error", line_location, "malformed @ai annotation"))
            continue

        key, raw_value = match.groups()
        if key not in KNOWN_KEYS:
            issues.append(Issue("error", line_location, f"unknown tag @ai.{key}"))
            continue
        if raw_value is None or not raw_value.strip():
            issues.append(Issue("error", line_location, f"@ai.{key} requires one value"))
            continue
        tags.append(Tag(key=key, value=raw_value.strip(), line=line_number))

    return tags, issues


def validate_tags(tags: Sequence[Tag], location: str, *, require_role: bool = True) -> list[Issue]:
    """Validate values, cardinality, and local evidence requirements."""

    issues: list[Issue] = []
    seen: dict[str, list[Tag]] = {}
    for tag in tags:
        seen.setdefault(tag.key, []).append(tag)
        tag_location = f"{location}:{tag.line}" if tag.line is not None else location

        if tag.key in {"role", "invariant", "law", "requires", "ensures"}:
            valid = KEBAB_RE.fullmatch(tag.value) is not None
            expected = "lowercase kebab-case"
        elif tag.key == "domain":
            valid = DOMAIN_RE.fullmatch(tag.value) is not None
            expected = "a lowercase dotted domain ID"
        elif tag.key == "pure":
            valid = tag.value in {"true", "false"}
            expected = "true or false"
        else:
            valid = RUST_PATH_RE.fullmatch(tag.value) is not None
            expected = "a Rust symbol path"

        if not valid:
            issues.append(
                Issue("error", tag_location, f"invalid @ai.{tag.key} value {tag.value!r}; expected {expected}")
            )

    for key in SINGLETON_KEYS:
        values = seen.get(key, [])
        if len(values) > 1:
            issues.append(Issue("error", location, f"@ai.{key} may appear at most once per item"))

    for key in REPEATABLE_KEYS:
        values = [tag.value for tag in seen.get(key, [])]
        duplicates = sorted({value for value in values if values.count(value) > 1})
        for value in duplicates:
            issues.append(
                Issue("error", location, f"duplicate @ai.{key} value {value!r} on the same item")
            )

    if (seen.get("invariant") or seen.get("law")) and not seen.get("evidence"):
        issues.append(
            Issue("error", location, "items with @ai.invariant or @ai.law require @ai.evidence")
        )

    if require_role and tags and "role" not in seen:
        issues.append(Issue("warning", location, "annotated item has no @ai.role"))

    return issues


def display_path(path: Path) -> str:
    """Prefer a repository-relative display path without requiring repository discovery."""

    resolved = path.resolve()
    try:
        return resolved.relative_to(Path.cwd().resolve()).as_posix()
    except ValueError:
        return resolved.as_posix()


def rust_files(paths: Sequence[Path]) -> list[Path]:
    """Resolve explicit files/directories to a deterministic Rust source list."""

    found: set[Path] = set()
    for path in paths:
        if not path.exists():
            raise FileNotFoundError(f"Rust source path does not exist: {path}")
        if path.is_file():
            if path.suffix != ".rs":
                raise ValueError(f"expected a Rust source file or directory, got: {path}")
            found.add(path.resolve())
        elif path.is_dir():
            for candidate in path.rglob("*.rs"):
                if {"target", ".git"}.intersection(candidate.parts):
                    continue
                found.add(candidate.resolve())
    return sorted(found, key=lambda item: item.as_posix())


def skip_attributes(lines: Sequence[str], start: int) -> int:
    """Skip blank lines and one or more possibly multi-line Rust attributes."""

    index = start
    while index < len(lines):
        if not lines[index].strip():
            index += 1
            continue
        if not lines[index].lstrip().startswith("#"):
            break

        balance = 0
        while index < len(lines):
            balance += lines[index].count("[") - lines[index].count("]")
            index += 1
            if balance <= 0:
                break
    return index


def identify_item(lines: Sequence[str], start: int) -> tuple[str, str, int, str]:
    """Identify the next common Rust item from a small signature window."""

    item_start = skip_attributes(lines, start)
    window_lines = lines[item_start : min(len(lines), item_start + 24)]
    signature = " ".join(line.strip() for line in window_lines)
    signature = signature.split("{", 1)[0].split(";", 1)[0]

    for kind, pattern in ITEM_PATTERNS:
        match = pattern.search(signature)
        if match is not None:
            visibility = (
                "public"
                if re.search(r"\bpub(?:\s*\([^)]*\))?", signature)
                else "private"
            )
            return kind, match.group(1), item_start + 1, visibility

    impl_match = re.search(r"\bimpl(?:\s*<[^>]+>)?\s+([^\s{]+)(?:\s+for\s+([^\s{]+))?", signature)
    if impl_match is not None:
        target = impl_match.group(2) or impl_match.group(1)
        return "impl", target, item_start + 1, "private"

    return "unknown", "<unattached>", item_start + 1, "unknown"


def source_contracts(paths: Sequence[Path]) -> tuple[list[Contract], list[Issue]]:
    """Collect annotated contracts from Rust source files."""

    contracts: list[Contract] = []
    issues: list[Issue] = []

    for path in rust_files(paths):
        display = display_path(path)
        lines = path.read_text(encoding="utf-8").splitlines()
        index = 0
        while index < len(lines):
            first = DOC_RE.match(lines[index])
            if first is None:
                index += 1
                continue

            doc_start = index
            doc_lines: list[tuple[int, str]] = []
            prefixes: set[str] = set()
            while index < len(lines):
                match = DOC_RE.match(lines[index])
                if match is None:
                    break
                prefixes.add(match.group("prefix"))
                doc_lines.append((index + 1, match.group("body")))
                index += 1

            if not any("@ai." in body for _, body in doc_lines):
                continue

            tags, parse_issues = parse_tags(doc_lines, display)
            issues.extend(parse_issues)

            if "//!" in prefixes:
                kind = "module"
                name = path.stem
                item_line = doc_start + 1
                visibility = "module"
            else:
                kind, name, item_line, visibility = identify_item(lines, index)

            location = f"{display}:{item_line}:{name}"
            issues.extend(validate_tags(tags, location))
            if kind == "unknown":
                issues.append(Issue("warning", location, "could not attach docs to a recognized Rust item"))

            item_id = f"source:{display}:{item_line}:{kind}:{name}"
            contracts.append(
                Contract(
                    item_id=item_id,
                    name=name,
                    kind=kind,
                    docs="\n".join(body for _, body in doc_lines),
                    tags=tags,
                    span={"filename": display, "begin": [item_line, 1]},
                    visibility=visibility,
                    attributes=[],
                    links={},
                    source="source",
                    source_path=display,
                    line=item_line,
                )
            )

    return contracts, issues


def rustdoc_kind(item: dict[str, Any]) -> str:
    """Extract the rustdoc-types item kind across common JSON layouts."""

    inner = item.get("inner")
    if isinstance(inner, dict) and inner:
        return str(next(iter(inner)))
    return str(item.get("kind", "unknown"))


def rustdoc_contracts(path: Path) -> tuple[list[Contract], list[Issue]]:
    """Collect annotated contracts from one rustdoc JSON artifact."""

    payload = json.loads(path.read_text(encoding="utf-8"))
    index = payload.get("index")
    if not isinstance(index, dict):
        raise ValueError("rustdoc JSON root must contain an object field named 'index'")

    contracts: list[Contract] = []
    issues: list[Issue] = []
    for raw_id, raw_item in sorted(index.items(), key=lambda pair: str(pair[0])):
        if not isinstance(raw_item, dict):
            continue
        docs = raw_item.get("docs")
        if not isinstance(docs, str) or "@ai." not in docs:
            continue

        rustdoc_id = str(raw_id)
        location = f"{path.as_posix()}#rustdoc:{rustdoc_id}"
        doc_lines = [(line_number, body) for line_number, body in enumerate(docs.splitlines(), 1)]
        tags, parse_issues = parse_tags(doc_lines, location)
        issues.extend(parse_issues)
        issues.extend(validate_tags(tags, location))

        name = raw_item.get("name")
        links = raw_item.get("links")
        attrs = raw_item.get("attrs", raw_item.get("attributes", []))
        contracts.append(
            Contract(
                item_id=f"rustdoc:{rustdoc_id}",
                rustdoc_id=rustdoc_id,
                name=str(name) if name is not None else "<anonymous>",
                kind=rustdoc_kind(raw_item),
                docs=docs,
                tags=tags,
                span=raw_item.get("span"),
                visibility=raw_item.get("visibility"),
                attributes=attrs,
                links=links if isinstance(links, dict) else {},
                source="rustdoc-json",
                source_path=path.as_posix(),
            )
        )

    return contracts, issues


def target_for(tag: Tag) -> tuple[str, str, str]:
    """Return edge type, target node ID, and target node type."""

    edge_type, namespace = EDGE_MAP[tag.key]
    if tag.key == "pure":
        target_id = f"property:pure={tag.value}"
        label = f"pure={tag.value}"
    else:
        target_id = f"{namespace}:{tag.value}"
        label = tag.value
    return edge_type, target_id, label


def build_graph(contracts: Sequence[Contract]) -> dict[str, Any]:
    """Build a deterministic property graph document."""

    nodes: dict[str, dict[str, Any]] = {}
    edges: list[dict[str, Any]] = []

    for contract in contracts:
        nodes[contract.item_id] = {
            "id": contract.item_id,
            "type": "rust_item",
            "name": contract.name,
            "kind": contract.kind,
            "span": contract.span,
            "visibility": contract.visibility,
            "docs": contract.docs,
            "attributes": contract.attributes,
            "links": contract.links,
            "source": contract.source,
            "rustdoc_id": contract.rustdoc_id,
        }

        for tag in contract.tags:
            edge_type, target_id, label = target_for(tag)
            target_type = target_id.split(":", 1)[0]
            nodes.setdefault(
                target_id,
                {"id": target_id, "type": target_type, "name": label},
            )
            edges.append(
                {
                    "source": contract.item_id,
                    "type": edge_type,
                    "target": target_id,
                    "annotation": f"@ai.{tag.key}",
                }
            )

        for link_text, raw_target in sorted(contract.links.items()):
            target_id = f"rustdoc:{raw_target}"
            nodes.setdefault(
                target_id,
                {
                    "id": target_id,
                    "type": "rust_item_ref",
                    "name": str(link_text),
                    "rustdoc_id": str(raw_target),
                },
            )
            edges.append(
                {
                    "source": contract.item_id,
                    "type": "rustdoc_link",
                    "target": target_id,
                    "label": str(link_text),
                }
            )

    return {
        "schema": "rust-ai-doc-contracts",
        "schema_version": SCHEMA_VERSION,
        "nodes": [nodes[key] for key in sorted(nodes)],
        "edges": sorted(
            edges,
            key=lambda edge: (edge["source"], edge["type"], edge["target"]),
        ),
    }


def dot_escape(value: Any) -> str:
    """Escape a value for a quoted Graphviz label."""

    return str(value).replace("\\", "\\\\").replace('"', '\\"').replace("\n", "\\n")


def graph_as_dot(graph: dict[str, Any]) -> str:
    """Render the graph as deterministic Graphviz DOT."""

    lines = ["digraph rust_ai_contracts {", "  rankdir=LR;"]
    for node in graph["nodes"]:
        label = node.get("name") or node["id"]
        kind = node.get("kind") or node.get("type")
        lines.append(
            f'  "{dot_escape(node["id"])}" [label="{dot_escape(label)}\\n{dot_escape(kind)}"];'
        )
    for edge in graph["edges"]:
        label = edge.get("label", edge["type"])
        lines.append(
            f'  "{dot_escape(edge["source"])}" -> "{dot_escape(edge["target"])}" '
            f'[label="{dot_escape(label)}"];'
        )
    lines.append("}")
    return "\n".join(lines)


def print_issues(issues: Sequence[Issue]) -> None:
    """Print issues in stable severity/location order."""

    for issue in sorted(issues, key=lambda item: (item.location, item.severity, item.message)):
        print(f"{issue.severity}: {issue.location}: {issue.message}", file=sys.stderr)


def issue_exit_code(issues: Sequence[Issue], fail_on_warnings: bool) -> int:
    """Convert issue severities to a CLI exit status."""

    has_error = any(issue.severity == "error" for issue in issues)
    has_warning = any(issue.severity == "warning" for issue in issues)
    return 1 if has_error or (fail_on_warnings and has_warning) else 0


def add_graph_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--format", choices=("json", "dot"), default="json")
    parser.add_argument("--pretty", action="store_true", help="Indent JSON output")
    parser.add_argument(
        "--fail-on-warnings",
        action="store_true",
        help="Return a failure status when source attachment/role warnings exist",
    )


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Validate and index @ai.* contracts in Rust documentation."
    )
    subparsers = parser.add_subparsers(dest="command", required=True)

    check = subparsers.add_parser("check", help="Validate annotations in Rust source paths")
    check.add_argument("paths", nargs="+", type=Path)
    check.add_argument("--fail-on-warnings", action="store_true")

    index = subparsers.add_parser("index", help="Index annotations from Rust source paths")
    index.add_argument("paths", nargs="+", type=Path)
    add_graph_arguments(index)

    rustdoc = subparsers.add_parser(
        "index-rustdoc", help="Index annotations and links from rustdoc JSON"
    )
    rustdoc.add_argument("json_path", type=Path)
    add_graph_arguments(rustdoc)
    return parser


def emit_graph(graph: dict[str, Any], output_format: str, pretty: bool) -> None:
    if output_format == "dot":
        print(graph_as_dot(graph))
    else:
        print(json.dumps(graph, indent=2 if pretty else None, sort_keys=True))


def main(argv: Sequence[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        if args.command in {"check", "index"}:
            contracts, issues = source_contracts(args.paths)
        else:
            contracts, issues = rustdoc_contracts(args.json_path)
    except (OSError, UnicodeError, json.JSONDecodeError, ValueError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 2

    print_issues(issues)
    status = issue_exit_code(issues, args.fail_on_warnings)
    if args.command == "check":
        error_count = sum(issue.severity == "error" for issue in issues)
        warning_count = sum(issue.severity == "warning" for issue in issues)
        print(
            f"checked {len(contracts)} annotated item(s): "
            f"{error_count} error(s), {warning_count} warning(s)"
        )
        return status

    if status != 0:
        return status
    emit_graph(build_graph(contracts), args.format, args.pretty)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
