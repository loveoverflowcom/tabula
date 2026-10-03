#!/usr/bin/env python3
"""Stage the #60 loopback-only prototype from pinned, local build inputs.

Never changes a production route, dependency lock, remote service or checkout.
Cargo builds are offline. A matching wasm-bindgen CLI is supplied explicitly.
"""
import argparse
import hashlib
import json
import os
import shutil
import struct
import subprocess
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def copy(source, destination):
    source = source.resolve(strict=True)
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, destination)
    return {"source": str(source), "path": str(destination),
            "sha256": sha(source), "bytes": source.stat().st_size}


def run(command, env=None):
    print(json.dumps({"kind": "build-command", "argv": command}), flush=True)
    subprocess.run(command, cwd=REPO, env=env, check=True)


def default_font():
    candidates = sorted((Path.home() / ".cargo/registry/src").glob("*/macroquad-0.4.16/src/ProggyClean.ttf"))
    if len(candidates) != 1:
        raise ValueError("Supply --font for the pinned Macroquad 0.4.16 default font")
    return candidates[0]


def verified_atlas_bytes(source_directory, authority_tool):
    """Return the exact in-memory bytes accepted by the Rust pack verifier."""
    data = {name: (source_directory / name).read_bytes()
            for name in ["tiles@1x.png", "tiles@2x.png"]}
    with tempfile.TemporaryDirectory(prefix="tabula60-atlas-snapshot-") as directory:
        snapshot = Path(directory)
        for name, content in data.items():
            (snapshot / name).write_bytes(content)
        result = subprocess.run([str(authority_tool.resolve(strict=True)), "--verify-assets", str(snapshot)],
                                cwd=REPO, capture_output=True, text=True, check=True)
        receipt = json.loads(result.stdout)
        if receipt.get("status") != "PASS" or len(receipt.get("files", [])) != 2:
            raise ValueError("Rust atlas verifier returned no complete PASS receipt")
    # Use those same original bytes, not another mutable read from the checkout.
    return data, receipt


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--destination", type=Path, default=Path("/tmp/tabula-issue-60-spike"))
    parser.add_argument("--target-dir", type=Path, default=Path(os.environ.get("CARGO_TARGET_DIR", REPO / "target")))
    parser.add_argument("--macroquad-wasm", type=Path)
    parser.add_argument("--fixture-dir", type=Path, default=HERE / "fixtures")
    parser.add_argument("--font", type=Path)
    parser.add_argument("--asset-source-dir", type=Path, default=REPO / "assets/packs/tiles")
    parser.add_argument("--authority-tool", type=Path)
    parser.add_argument("--leptos-wasm", type=Path)
    parser.add_argument("--wasm-bindgen", type=Path)
    parser.add_argument("--build-leptos", action="store_true")
    args = parser.parse_args()
    destination = args.destination.resolve()
    target = args.target_dir.resolve()
    authority_tool = args.authority_tool or target / "debug/examples/embedding_fixture"
    atlases, atlas_verification = verified_atlas_bytes(args.asset_source_dir, authority_tool)
    destination.mkdir(parents=True, exist_ok=True)
    wasm = args.macroquad_wasm or target / "wasm32-unknown-unknown/wasm-release/examples/tiles_renderer_baseline.wasm"
    receipts = []
    for source in sorted(HERE.iterdir()):
        if source.suffix in {".mjs", ".html", ".css"}:
            receipts.append(copy(source, destination / source.name))
    receipts.append(copy(REPO / "apps/web/style/tokens.css", destination / "tokens.css"))
    receipts.append(copy(REPO / "apps/game-client/web/mq_js_bundle.js", destination / "mq_js_bundle.js"))
    receipts.append(copy(wasm, destination / "tabula-game-client.wasm"))
    for name in ["fixture.json", "render-contract.json"]:
        receipts.append(copy(args.fixture_dir / name, destination / name))
    fixture = json.loads((destination / "fixture.json").read_text())
    assets = []
    for file in fixture["assets"]["files"]:
        verified = next((value for value in atlas_verification["files"] if value["name"] == file["name"]), None)
        if verified is None or any(verified[key] != file[key] for key in ["name", "path", "hash", "bytes", "density"]):
            raise ValueError(f"Fixture metadata differs from native canonical pack: {file['name']}")
        source = args.asset_source_dir / f"tiles@{file['density']}x.png"
        data = atlases[source.name]
        if len(data) != file["bytes"]:
            raise ValueError(f"Current asset byte size differs from verified Rust fixture: {file['name']}")
        target_file = destination / file["path"]
        target_file.parent.mkdir(parents=True, exist_ok=True)
        target_file.write_bytes(data)
        receipt = {"source": str(source.resolve()), "path": str(target_file), "sha256": hashlib.sha256(data).hexdigest(), "bytes": len(data)}
        receipts.append(receipt)
        if data[:8] != b"\x89PNG\r\n\x1a\n":
            raise ValueError("Fixture atlas must be a PNG")
        width, height = struct.unpack(">II", data[16:24])
        assets.append({"name": file["name"], "path": file["path"], "sha256": receipt["sha256"],
                       "bytes": receipt["bytes"], "width": width, "height": height,
                       "blake3": file["hash"], "blake3_provenance": "Rust fixed-scope verifier checks this exact staged byte snapshot against the canonical pack"})
    receipts.append(copy(REPO / "assets/packs/tiles/LICENSE", destination / "licenses/tiles-LICENSE"))
    pixi = HERE / "node_modules/pixi.js"
    for name in ["dist/pixi.mjs", "LICENSE", "package.json"]:
        receipts.append(copy(pixi / name, destination / "node_modules/pixi.js" / name))
    font_source = args.font or default_font()
    font_receipt = copy(font_source, destination / "fonts/ProggyClean.ttf")
    receipts.append(font_receipt)
    for name in ["LICENSE-MIT", "LICENSE-APACHE"]:
        receipts.append(copy(font_source.parent.parent / name, destination / "licenses" / f"macroquad-{name}"))
    for source in sorted((HERE / "licenses").rglob("*")):
        if source.is_file():
            receipts.append(copy(source, destination / "licenses" / source.relative_to(HERE / "licenses")))
    font = {"url": "fonts/ProggyClean.ttf", "sha256": font_receipt["sha256"],
            "bytes": font_receipt["bytes"], "family": "TabulaSpikeProggy",
            "license": "licenses/ProggyClean-MIT.txt",
            "source": "Pinned macroquad 0.4.16 src/ProggyClean.ttf; same default font as #59"}
    if args.build_leptos:
        env = {**os.environ, "CARGO_TARGET_DIR": str(target)}
        run(["cargo", "build", "--offline", "-p", "tabula-web", "--example", "renderer_embedding_shell",
             "--target", "wasm32-unknown-unknown", "--profile", "wasm-release"], env)
    leptos_wasm = args.leptos_wasm or target / "wasm32-unknown-unknown/wasm-release/examples/renderer_embedding_shell.wasm"
    leptos = None
    if args.wasm_bindgen:
        version = subprocess.check_output([str(args.wasm_bindgen), "--version"], text=True).strip()
        if version != "wasm-bindgen 0.2.129":
            raise ValueError(f"CLI version must match Cargo.lock wasm-bindgen 0.2.129, got {version}")
        run([str(args.wasm_bindgen), str(leptos_wasm.resolve(strict=True)), "--target", "web",
             "--out-dir", str(destination / "leptos"), "--out-name", "renderer_embedding_shell"])
        leptos = {"wasm_bindgen_version": version, "input_wasm_sha256": sha(leptos_wasm),
                  "output_files": [{"path": str(p.relative_to(destination)), "sha256": sha(p), "bytes": p.stat().st_size}
                                   for p in sorted((destination / "leptos").rglob("*")) if p.is_file()]}
    elif args.build_leptos:
        raise ValueError("--build-leptos requires --wasm-bindgen with the matching local CLI")
    commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=REPO, text=True).strip()
    runtime = {"schema_version": 1, "source_git_sha": commit, "fixture_source_revision": fixture["source_revision"],
               "assets": assets, "font": font, "leptos": leptos,
               "atlas_verification": atlas_verification,
               "native_verifier": {"path": str(authority_tool.resolve()), "sha256": sha(authority_tool), "bytes": authority_tool.stat().st_size},
               "files": receipts, "distribution": "loopback staging only; not a production artifact"}
    (destination / "runtime-assets.json").write_text(json.dumps(runtime, indent=2) + "\n")
    print(json.dumps({"kind": "staged", "directory": str(destination), "source_git_sha": commit,
                      "fixture_sha256": sha(destination / "fixture.json"), "leptos_built": leptos is not None,
                      "files": len(receipts), "assets": len(assets)}), flush=True)


if __name__ == "__main__":
    main()
