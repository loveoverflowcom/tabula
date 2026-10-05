#!/usr/bin/env python3
"""Inspect actual emitted bundles; never infer browser timing or cache hits.

The local Chess host has a small reproducible download budget and must not
inline its external resources or link unrelated games. A shell distribution
is optional because the regular CI job compiles, but does not bundle, Leptos.
"""

import argparse
import gzip
import hashlib
import json
from pathlib import Path
import re


ROOT = Path(__file__).resolve().parents[2]
GAME_RAW_LIMIT = 1_250_000
GAME_GZIP_LIMIT = 500_000
SHELL_RAW_LIMIT = 900_000


def receipt(path):
    payload = path.read_bytes()
    return {
        "path": path.name,
        "bytes": len(payload),
        "gzip9_bytes": len(gzip.compress(payload, compresslevel=9, mtime=0)),
        "sha256": hashlib.sha256(payload).hexdigest(),
    }


def game_budget(path, graph):
    payload = path.read_bytes()
    assert payload.startswith(b"\0asm\x01\0\0\0"), "not a WASM module"
    result = receipt(path)
    assert result["bytes"] <= GAME_RAW_LIMIT, f"game raw size exceeds {GAME_RAW_LIMIT}: {result['bytes']}"
    assert result["gzip9_bytes"] <= GAME_GZIP_LIMIT, f"game gzip size exceeds {GAME_GZIP_LIMIT}: {result['gzip9_bytes']}"
    checked = []
    for directory in ("assets/fonts", "games/chess/assets", "games/tiles/assets"):
        for file in sorted((ROOT / directory).iterdir()):
            if file.suffix not in (".png", ".ttf"):
                continue
            assert file.read_bytes() not in payload, f"external/unrelated payload inlined: {file}"
            checked.append(str(file.relative_to(ROOT)))
    result["external_payloads_absent"] = checked
    if graph:
        text = graph.read_text()
        assert "tabula-game-client " in text and "tabula-game-chess " in text, "game graph is empty/incomplete"
        assert "tabula-game-tiles " not in text, "unrelated game linked in deployed WASM graph"
        assert "leptos " not in text, "DOM runtime linked in gameplay graph (I-15)"
        result["normal_graph_check"] = "PASS: selected game only, no DOM runtime"
    return result


def shell_budget(directory):
    html = (directory / "index.html").read_text()
    # Trunk emits imports and preload links for the same two shell artifacts.
    urls = set(re.findall(r'(?:src|href)="(/[^"?#]+)"', html))
    urls.update(re.findall(r"(?:from\s+|module_or_path:\s*)'(/[^'?#]+)'", html))
    assert urls, "no shell resources selected"
    assert all(not url.startswith(("/play/", "/resources/", "/assets/")) for url in urls), "shell eagerly references gameplay resources"
    rows = [receipt(directory / "index.html")]
    for url in sorted(urls):
        file = directory / url.removeprefix("/")
        assert file.is_file(), f"shell reference missing: {url}"
        rows.append(receipt(file))
    wasm = [row for row in rows if row["path"].endswith(".wasm")]
    assert len(wasm) == 1, "shell must download exactly its own WASM module"
    assert wasm[0]["bytes"] <= SHELL_RAW_LIMIT, "shell WASM raw budget exceeded"
    for css in directory.glob("*.css"):
        assert not re.search(r"url\s*\(\s*['\"]?(?:https?://|/play/|/resources/|[^)]*\.(?:png|ttf|wasm))", css.read_text()), "shell CSS eagerly references external/game assets"
    return {
        "evidence": "static emitted dependency inventory, not browser request waterfall",
        "unique_resource_count_including_document": len(rows),
        "raw_bytes": sum(row["bytes"] for row in rows),
        "individual_gzip9_sum": sum(row["gzip9_bytes"] for row in rows),
        "resources": rows,
        "gameplay_asset_references": 0,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--game-wasm", type=Path, required=True)
    parser.add_argument("--game-tree", type=Path)
    parser.add_argument("--shell-dist", type=Path)
    parser.add_argument("--write-receipt", type=Path)
    args = parser.parse_args()
    result = {"game": game_budget(args.game_wasm, args.game_tree)}
    if args.shell_dist:
        result["shell"] = shell_budget(args.shell_dist)
    encoded = json.dumps(result, indent=2) + "\n"
    if args.write_receipt:
        args.write_receipt.write_text(encoded)
    print(encoded, end="")
    print("PASS: emitted loading budgets (not runtime performance)")


if __name__ == "__main__":
    main()
