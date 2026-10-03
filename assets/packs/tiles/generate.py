#!/usr/bin/env python3
"""Original Tiles fixture art; standard-library, deterministic RGBA PNG/SVG authoring.

The shapes below are the editable source. White coverage is tinted by semantic
design tokens at runtime; no artwork palette becomes a second theme owner.
"""
from pathlib import Path
import math
import struct
import zlib

ROOT = Path(__file__).resolve().parent
CELL, PAD, COLUMNS = 72, 4, 6
N, E, S, W = 0, 1, 2, 3
TILES = [
    ("city-cap-road", [[N]], [[E, W]], False, False),
    ("monastery", [], [], True, False),
    ("monastery-road", [], [[S]], True, False),
    ("road-straight", [], [[N, S]], False, False),
    ("road-curve", [], [[N, W]], False, False),
    ("road-junction", [], [[N], [E], [W]], False, False),
    ("road-crossroad", [], [[N], [E], [S], [W]], False, False),
    ("city-cap", [[N]], [], False, False),
    ("city-cap-crossroad", [[N]], [[E], [S], [W]], False, False),
    ("city-through", [[N, S]], [], False, False),
    ("city-corner", [[N, W]], [], False, False),
    ("city-corner-pennant", [[N, W]], [], False, True),
    ("city-corner-road", [[N, W]], [[E, S]], False, False),
    ("city-corner-road-pennant", [[N, W]], [[E, S]], False, True),
    ("city-three", [[N, E, W]], [], False, False),
    ("city-three-pennant", [[N, E, W]], [], False, True),
    ("city-three-road", [[N, E, W]], [[S]], False, False),
    ("city-three-road-pennant", [[N, E, W]], [[S]], False, True),
    ("city-full", [[N, E, S, W]], [], False, True),
    ("city-two-opposite", [[N], [S]], [], False, False),
    ("city-two-adjacent", [[N], [E]], [], False, False),
    ("city-cap-road-right", [[N]], [[E, S]], False, False),
    ("city-cap-road-left", [[N]], [[S, W]], False, False),
]


def rotate(point, side):
    x, y = point
    for _ in range(side):
        x, y = 64 - y, x
    return x, y


def geometry(tile):
    _, cities, roads, monastery, pennant = tile
    shapes = []
    # Corner gaps are deliberate: two separate adjacent cities must not appear
    # connected. Only the explicit multi-edge connector below joins cities.
    cap = [(8, 2), (56, 2), (50, 8), (44, 8), (40, 16),
           (24, 16), (20, 8), (14, 8)]
    for city in cities:
        if len(city) == 4:
            shapes.append(("polygon", [(2, 2), (62, 2), (62, 62), (2, 62)], 255))
        else:
            for side in city:
                shapes.append(("polygon", [rotate(p, side) for p in cap], 255))
            # Multiple sides in ONE city visibly connect. Separate caps do not.
            if len(city) > 1:
                for side in city:
                    shapes.append(("line", [rotate((32, 10), side), (32, 32), 22], 255))
        for side in city:
            # Stone slots have shape/texture cues even with colour unavailable.
            for x in [13, 25, 37, 49]:
                shapes.append(("line", [rotate((x, 4), side), rotate((x, 9), side), 2], 0))
    ends = [(32, 2), (62, 32), (32, 62), (2, 32)]
    for road in roads:
        if len(road) == 1:
            path = [ends[road[0]], (32, 32)]
        else:
            path = [ends[road[0]], (32, 32), ends[road[1]]]
        for a, b in zip(path, path[1:]):
            shapes.append(("line", [a, b, 8], 255))
            # Hollow lane distinguishes roads from city blocks.
            shapes.append(("line", [a, b, 3], 0))
    if any(len(road) == 1 for road in roads) and not monastery:
        # Junction marker: disconnected roads end at this town square.
        shapes.append(("polygon", [(25, 25), (39, 25), (39, 39), (25, 39)], 255))
        shapes.append(("polygon", [(28, 28), (36, 28), (36, 36), (28, 36)], 0))
    if monastery:
        shapes.append(("polygon", [(19, 29), (32, 17), (45, 29), (43, 29),
                                     (43, 45), (21, 45), (21, 29)], 255))
        shapes.append(("polygon", [(29, 34), (35, 34), (35, 45), (29, 45)], 0))
        shapes.append(("line", [(32, 21), (32, 29), 2], 0))
        shapes.append(("line", [(28, 25), (36, 25), 2], 0))
    if pennant:
        # A cut-out diamond/flag is readable independently of colour.
        shapes.append(("polygon", [(32, 20), (39, 28), (32, 36), (25, 28)], 0))
        shapes.append(("line", [(32, 23), (32, 33), 2], 255))
        shapes.append(("polygon", [(32, 23), (36, 26), (32, 28)], 255))
    return shapes


def inside_polygon(x, y, points):
    inside = False
    previous = points[-1]
    for point in points:
        ax, ay = previous
        bx, by = point
        if (ay > y) != (by > y) and x < (bx - ax) * (y - ay) / (by - ay) + ax:
            inside = not inside
        previous = point
    return inside


def inside(shape, x, y):
    kind, data, _ = shape
    if kind == "polygon":
        return inside_polygon(x, y, data)
    a, b, width = data
    dx, dy = b[0] - a[0], b[1] - a[1]
    t = max(0, min(1, ((x - a[0]) * dx + (y - a[1]) * dy) / (dx * dx + dy * dy)))
    return math.hypot(x - a[0] - t * dx, y - a[1] - t * dy) <= width / 2


def png(density):
    width, height = CELL * COLUMNS * density, CELL * 4 * density
    pixels = bytearray(width * height * 4)
    for index, tile in enumerate(TILES):
        ox, oy = ((index % COLUMNS) * CELL + PAD) * density, ((index // COLUMNS) * CELL + PAD) * density
        shapes = geometry(tile)
        for y in range(64 * density):
            for x in range(64 * density):
                coverage = 0
                for sy, sx in [(0.25, 0.25), (0.25, 0.75), (0.75, 0.25), (0.75, 0.75)]:
                    alpha = 0
                    for shape in shapes:
                        if inside(shape, (x + sx) / density, (y + sy) / density):
                            alpha = shape[2]
                    coverage += alpha
                offset = ((oy + y) * width + ox + x) * 4
                pixels[offset:offset + 4] = bytes([255, 255, 255, round(coverage / 4)])
    raw = b"".join(b"\x00" + pixels[y * width * 4:(y + 1) * width * 4] for y in range(height))
    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b""))


def svg():
    output = ['<svg xmlns="http://www.w3.org/2000/svg" width="432" height="288" viewBox="0 0 432 288">',
              '<title>Original Tabula Tiles fixture atlas — CC0-1.0</title>', '<defs>']
    for index, tile in enumerate(TILES):
        output.append(f'<mask id="m{index}" maskUnits="userSpaceOnUse" x="0" y="0" width="64" height="64">')
        for kind, data, alpha in geometry(tile):
            color = "white" if alpha else "black"
            if kind == "polygon":
                points = " ".join(f"{x},{y}" for x, y in data)
                output.append(f'<polygon points="{points}" fill="{color}"/>')
            else:
                a, b, width = data
                output.append(f'<line x1="{a[0]}" y1="{a[1]}" x2="{b[0]}" y2="{b[1]}" stroke="{color}" stroke-width="{width}" stroke-linecap="round"/>')
        output.append('</mask>')
    output.append('</defs>')
    for index, tile in enumerate(TILES):
        x, y = (index % COLUMNS) * CELL + PAD, (index // COLUMNS) * CELL + PAD
        output.append(f'<g id="{tile[0]}" transform="translate({x} {y})"><rect width="64" height="64" fill="white" mask="url(#m{index})"/></g>')
    return "\n".join(output + ['</svg>', ''])


def manifest_source():
    output = ["# Generated by generate.py; editable shapes are in that file and tiles.svg."]
    for density in [1, 2]:
        output += ["", "[[files]]", f'name = "tiles@{density}x.atlas"',
                   f'source = "tiles@{density}x.png"', 'priority = "critical"', f'density = {density}']
    for index, tile in enumerate(TILES):
        output += ["", "[[resources]]", f'id = "tiles/{tile[0]}"']
        for density in [1, 2]:
            x, y = ((index % COLUMNS) * CELL + PAD) * density, ((index // COLUMNS) * CELL + PAD) * density
            output += ["", "[[resources.variants]]", f'file = "tiles@{density}x.atlas"',
                       f'region = {{ x = {x}, y = {y}, width = {64 * density}, height = {64 * density} }}']
    return "\n".join(output) + "\n"


if __name__ == "__main__":
    for density in [1, 2]:
        (ROOT / f"tiles@{density}x.png").write_bytes(png(density))
    (ROOT / "tiles.svg").write_text(svg())
    (ROOT / "pack.source.toml").write_text(manifest_source())
    print("Wrote original SVG, 1x/2x RGBA PNG atlas and explicit source manifest.")
