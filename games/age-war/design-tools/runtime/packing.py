"""Pure image/packing helpers for D05 runtime packs. Presentation data only.

Nothing here reads delivery archives or decides combat. Inputs are Pillow
images and plain records; outputs are images and plain records. The builder
calls these one atlas page at a time so source pages never coexist in memory.
"""
from __future__ import annotations

from dataclasses import dataclass
import io

import numpy as np
from PIL import Image, ImageChops

PAGE = 2048
EXTRUDE = 2  # copied edge texels for linear filtering
GAP = 2      # transparent texels outside the extrusion ring
PAD = EXTRUDE + GAP


def scaled_size(width: int, height: int, scale: float) -> tuple[int, int]:
    if not 0 < scale <= 1:
        raise ValueError('runtime frames are only downsampled, never invented')
    return max(1, round(width * scale)), max(1, round(height * scale))


def premultiplied_resize(image: Image.Image, size: tuple[int, int]) -> Image.Image:
    """Lanczos resize without dark fringes: resample premultiplied colour."""
    if image.size == size:
        return image.copy()
    return image.convert('RGBa').resize(size, Image.Resampling.LANCZOS).convert('RGBA')


def shaded_mask(colour: Image.Image, mask: Image.Image) -> Image.Image:
    """Team-paint mask: luminance-shaded white limited to the colour silhouette.

    A runtime tint multiplies this shading, so painted cloth keeps its folds.
    Alpha is the minimum of mask and colour alpha, so paint cannot leave the
    silhouette even if a delivered mask texel does.
    """
    if colour.size != mask.size:
        raise ValueError('mask and colour frames must share one rectangle')
    r, g, b, a = colour.split()
    shade = Image.merge('RGB', (r, g, b)).convert('L').point(lambda v: min(255, int(v * 1.15 + 25)))
    alpha = ImageChops.darker(mask.getchannel('A'), a)
    return Image.merge('RGBA', (shade, shade, shade, alpha))


def alpha_bbox(image: Image.Image) -> tuple[int, int, int, int] | None:
    return image.getchannel('A').getbbox()


@dataclass(frozen=True)
class Placement:
    key: tuple
    page: int
    x: int
    y: int
    width: int
    height: int


def shelf_pack(items: list[tuple[tuple, int, int]],
               page: int = PAGE) -> tuple[list[Placement], list[tuple[int, int]]]:
    """Deterministic shelf packing of (key, width, height) items.

    Each item reserves PAD texels on every side. Sorting by height, width and
    key makes the result independent of declaration order. Returns placements
    and the used (width, height) of every page, height rounded up to 4 texels.
    """
    ordered = sorted(items, key=lambda item: (-item[2], -item[1], item[0]))
    placements, pages = [], []
    page_index, x, y, shelf, used_height = 0, 0, 0, 0, 0
    for key, width, height in ordered:
        cell_w, cell_h = width + 2 * PAD, height + 2 * PAD
        if cell_w > page or cell_h > page:
            raise ValueError(f'frame {key} cannot fit a {page} page')
        if x + cell_w > page:
            x, y, shelf = 0, y + shelf, 0
        if y + cell_h > page:
            pages.append((page, _round4(used_height)))
            page_index, x, y, shelf, used_height = page_index + 1, 0, 0, 0, 0
        placements.append(Placement(key, page_index, x + PAD, y + PAD, width, height))
        x += cell_w
        shelf = max(shelf, cell_h)
        used_height = max(used_height, y + cell_h)
    if placements:
        pages.append((page, _round4(used_height)))
    return placements, pages


def _round4(value: int) -> int:
    return min(PAGE, (value + 3) // 4 * 4)


def extruded(frame: Image.Image, extrude: int = EXTRUDE) -> Image.Image:
    """Frame plus an `extrude`-texel ring of clamped edge texels."""
    array = np.asarray(frame.convert('RGBA'))
    return Image.fromarray(np.pad(array, ((extrude, extrude), (extrude, extrude), (0, 0)),
                                  mode='edge'), 'RGBA')


def paste_extruded(page: Image.Image, frame: Image.Image, x: int, y: int) -> None:
    page.paste(extruded(frame), (x - EXTRUDE, y - EXTRUDE))


def extrusion_matches(page: Image.Image, x: int, y: int, width: int, height: int,
                      extrude: int = EXTRUDE) -> bool:
    """Independent check: every ring texel equals its clamped edge texel."""
    if x < extrude or y < extrude or x + width + extrude > page.width \
            or y + height + extrude > page.height:
        return False
    region = np.asarray(page.crop((x - extrude, y - extrude,
                                   x + width + extrude, y + height + extrude)).convert('RGBA'))
    inner = region[extrude:extrude + height, extrude:extrude + width]
    expected = np.pad(inner, ((extrude, extrude), (extrude, extrude), (0, 0)), mode='edge')
    return bool(np.array_equal(region, expected))


def png_bytes(image: Image.Image) -> bytes:
    """Deterministic lossless encoding for one Pillow version."""
    out = io.BytesIO()
    image.save(out, format='PNG', optimize=False, compress_level=9)
    return out.getvalue()
