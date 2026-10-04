#!/usr/bin/env python3
"""Check game-owned pack source locations without building Rust or generating art.

Runtime paths/hashes remain manifest-owned; the existing Rust pack tests verify
BLAKE3 integrity. This check guards source placement and pinned file metadata only.
"""

from pathlib import Path
import tomllib


ROOT = Path(__file__).resolve().parents[2]


def check(root):
    legacy = root / "assets" / "packs"
    assert not legacy.exists(), f"game pack sources must live under games/<game>/assets, found {legacy}"
    assert not list((root / "apps/game-client/web/assets").glob("chess-cover*")), "standalone covers/provenance must use game-owned sources"
    packs = 0
    files = 0
    for game_manifest in sorted((root / "games").glob("*/game.toml")):
        game = tomllib.loads(game_manifest.read_text())
        identity = game.get("assets", {}).get("pack")
        if not identity:
            continue
        global_game_assets = root / "assets" / game_manifest.parent.name
        assert not global_game_assets.exists(), f"game sources must stay beside their owner: {global_game_assets}"
        assets = game_manifest.parent / "assets"
        for name in ("README.md", "generate.py", "pack.source.toml", "fixture.pack.toml"):
            assert (assets / name).is_file(), f"missing game-owned pack input: {assets / name}"
        source = tomllib.loads((assets / "pack.source.toml").read_text())
        fixture = tomllib.loads((assets / "fixture.pack.toml").read_text())
        assert f"{fixture['pack']}@{fixture['version']}" == identity, f"pack identity mismatch: {assets}"
        declared = {file["name"]: file for file in fixture["files"]}
        assert len(declared) == len(fixture["files"]), f"duplicate fixture file names: {assets}"
        assert {file["name"] for file in source["files"]} == set(declared), f"source/fixture file set mismatch: {assets}"
        for file in source["files"]:
            path = assets / file["source"]
            assert path.resolve().is_relative_to(assets.resolve()), f"source escapes its game: {path}"
            assert path.is_file(), f"missing game-owned source: {path}"
            metadata = declared[file["name"]]
            assert path.stat().st_size == metadata["bytes"], f"source size differs from fixture: {path}"
            assert all(file.get(key) == metadata.get(key) for key in ("density", "priority")), f"source/fixture selection mismatch: {path}"
            files += 1
        packs += 1
    assert packs, "no configured game pack sources checked"
    return packs, files


if __name__ == "__main__":
    packs, files = check(ROOT)
    print(f"PASS: {packs} game-owned pack sources and {files} pinned source files; no legacy global game-art sources")
