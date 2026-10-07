#!/usr/bin/env python3
"""Bounded PNG exports of the licensed Chessnut artwork (doc 04 §12).

Requires Inkscape and Pillow only while regenerating; the Rust runtime decodes
PNG and has no new SVG, image-authoring or Python dependency. Original sources
remain unchanged under source/. The original cover is never embedded in Rust.
"""
from argparse import ArgumentParser
from io import BytesIO
from pathlib import Path
import hashlib
import json
import subprocess
import tempfile
import xml.etree.ElementTree as ET

from PIL import Image

ROOT = Path(__file__).resolve().parent
CELL, PAD, REGION, COLUMNS = 72, 4, 64, 6
PIECES = [
    (side + key, f"pieces/{color}-{kind}")
    for side, color in [("w", "white"), ("b", "black")]
    for key, kind in [("K", "king"), ("Q", "queen"), ("B", "bishop"),
                      ("N", "knight"), ("R", "rook"), ("P", "pawn")]
]
NOTICES = ["COPYRIGHT.txt", "LICENSE-Apache-2.0.txt", "NOTICE.txt", "PIECE-PROVENANCE.md"]


def validate_piece_sources():
    """Check all twelve pinned source bytes and self-contained square SVGs."""
    source = ROOT / "source" / "pieces"
    manifest = json.loads((source / "manifest.json").read_text())
    if (manifest["source_repository"] != "lichess-org/lila"
            or manifest["source_commit"] != "5820854ca42e082891d6a47ceb3b57d33ebb7fe1"
            or manifest["source_path"] != "public/piece/chessnut"
            or manifest["license"] != "Apache-2.0"):
        raise ValueError("unexpected Chessnut provenance")
    entries = {entry["file"]: entry for entry in manifest["pieces"]}
    if len(manifest["pieces"]) != 12 or set(entries) != {f"{key}.svg" for key, _ in PIECES}:
        raise ValueError("Chessnut source manifest must contain exactly twelve pieces")
    for filename, entry in entries.items():
        data = (source / filename).read_bytes()
        blob = hashlib.sha1(f"blob {len(data)}\0".encode() + data).hexdigest()
        if (len(data) != entry["bytes"] or hashlib.sha256(data).hexdigest() != entry["sha256"]
                or blob != entry["upstream_blob_sha"]):
            raise ValueError(f"source differs from pinned Chessnut bytes: {filename}")
        svg = ET.fromstring(data)
        if svg.tag != "{http://www.w3.org/2000/svg}svg" or svg.get("viewBox") != "0 0 800 800":
            raise ValueError(f"unexpected Chessnut square viewBox: {filename}")
        for element in svg.iter():
            if element.tag.split("}")[-1] not in {"svg", "g", "path", "circle", "ellipse"}:
                raise ValueError(f"unsupported SVG element: {filename}")
            for name, value in element.attrib.items():
                if name.lower().startswith("on") or "href" in name or "url(" in value.lower():
                    raise ValueError(f"external or active SVG content: {filename}")
    for name in NOTICES:
        if not (source / name).read_bytes():
            raise ValueError(f"missing Chessnut attribution: {name}")


def png_bytes(image):
    output = BytesIO()
    image.save(output, "PNG", compress_level=9)
    return output.getvalue()


def piece_atlas(density):
    atlas = Image.new("RGBA", (CELL * COLUMNS * density, CELL * 2 * density))
    with tempfile.TemporaryDirectory(prefix="tabula-chess-") as directory:
        for index, (key, _) in enumerate(PIECES):
            output = Path(directory) / f"{key}.png"
            subprocess.run([
                "inkscape", str(ROOT / "source" / "pieces" / f"{key}.svg"),
                "--export-type=png", f"--export-filename={output}",
                f"--export-width={REGION * density}", "--export-background-opacity=0",
            ], check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            with Image.open(output) as exported:
                image = exported.convert("RGBA")
                if image.size != (REGION * density, REGION * density):
                    raise ValueError(f"unexpected export size for {key}: {image.size}")
                # Paste straight-alpha RGBA, without another alpha mask. A mask
                # here would apply edge coverage twice and thin the contours.
                atlas.paste(image, ((index % COLUMNS * CELL + PAD) * density,
                                    (index // COLUMNS * CELL + PAD) * density))
    # Every cell retains four transparent logical pixels on each side. Leave
    # straight-alpha antialiased edge coverage intact; no tint or shape cropping.
    for y in range(atlas.height):
        for x in range(atlas.width):
            alpha = atlas.getpixel((x, y))[3]
            in_region = (PAD * density <= x % (CELL * density) < (PAD + REGION) * density
                         and PAD * density <= y % (CELL * density) < (PAD + REGION) * density)
            if not in_region and alpha:
                raise ValueError(f"nontransparent atlas gutter at {x}, {y}")
    return png_bytes(atlas)


def cover(density):
    with Image.open(ROOT / "source" / "chess-atmosphere.png") as original:
        image = original.convert("RGB").resize((240 * density, 160 * density),
                                                Image.Resampling.LANCZOS)
        return png_bytes(image)


def manifest_source():
    lines = ["# Generated by generate.py from retained canonical source artwork."]
    for name, prefix, priority in [("pieces", "pieces", "critical"),
                                    ("cover", "cover", "high")]:
        for density in [1, 2]:
            lines += ["", "[[files]]", f'name = "{name}@{density}x.atlas"',
                      f'source = "{prefix}@{density}x.png"',
                      f'priority = "{priority}"', f'density = {density}']
    for name in NOTICES:
        lines += ["", "[[files]]", f'name = "{name}"',
                  f'source = "source/pieces/{name}"', 'priority = "low"']
    for index, (_, resource) in enumerate(PIECES):
        lines += ["", "[[resources]]", f'id = "{resource}"']
        for density in [1, 2]:
            x = (index % COLUMNS * CELL + PAD) * density
            y = (index // COLUMNS * CELL + PAD) * density
            lines += ["", "[[resources.variants]]", f'file = "pieces@{density}x.atlas"',
                      f'region = {{ x = {x}, y = {y}, width = {REGION * density}, height = {REGION * density} }}']
    lines += ["", "[[resources]]", 'id = "catalog/cover"']
    for density in [1, 2]:
        lines += ["", "[[resources.variants]]", f'file = "cover@{density}x.atlas"']
    return "\n".join(lines) + "\n"


def main():
    parser = ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="verify exports without writing")
    args = parser.parse_args()
    validate_piece_sources()
    exports = {"pack.source.toml": manifest_source().encode()}
    for density in [1, 2]:
        exports[f"pieces@{density}x.png"] = piece_atlas(density)
        exports[f"cover@{density}x.png"] = cover(density)
    for name, data in exports.items():
        if args.check:
            if not (ROOT / name).is_file() or (ROOT / name).read_bytes() != data:
                raise SystemExit(f"stale Chess artwork: {name}")
        else:
            (ROOT / name).write_bytes(data)
    print(f"{'Verified' if args.check else 'Wrote'} canonical Chess 1x/2x PNG exports and source manifest")


if __name__ == "__main__":
    main()
