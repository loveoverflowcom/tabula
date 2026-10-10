"""Per-age pose sheets from the sealed runtime packs (private output).

Rows are unit x facing; columns are idle, locomotion, attack (windup start,
marker pose, end), first skill marker pose, hit and final death pose. Frames
come from the density-1 atlases and their manifest regions, drawn at their
logical size on the age's D03 backdrop strip, with the team mask tinted as in
the pilot. A second sheet repeats the same cells at 2x nearest for inspection.
Pixels shown here are what the runtime packs contain; motion quality between
these poses still needs the pilot captures and owner review.
"""
from __future__ import annotations

import argparse
from pathlib import Path
import tomllib

from PIL import Image, ImageChops, ImageDraw

AGES = ('Primitive', 'Ancient', 'Feudal', 'Arcane', 'Industrial', 'Future')
TEAM = {'right': (0, 95, 115), 'left': (202, 103, 2)}  # tokens.toml team[0]/team[1], light scheme
CELL = (150, 150)
COLUMNS = ('idle', 'move', 'attack:start', 'attack:marker', 'attack:end', 'skill:marker', 'hit', 'death:end')


def load_pack(root: Path, age: str) -> tuple[dict, dict, Path]:
    pack_root = next((root / f'age-war-{age.lower()}').iterdir())
    manifest = tomllib.loads((pack_root / 'pack.toml').read_text())
    files = {f['name']: f for f in manifest['files']}
    regions = {}
    for resource in manifest['resources']:
        for variant in resource['variants']:
            density = files[variant['file']].get('density')
            if density in (1, None):
                regions[resource['id']] = (variant['file'], variant.get('region'))
    return files, regions, pack_root.parent.parent


def parse_clips(text: str) -> tuple[list, dict, dict]:
    units, clips, frames = [], {}, {}
    for line in text.splitlines():
        f = line.split('\t')
        if f[0] == 'unit':
            units.append(f[1])
        elif f[0] == 'clip':
            clips[(f[1], f[2], f[3])] = {'skill': f[4], 'duration': float(f[5]), 'windup': f[7], 'markers': []}
        elif f[0] == 'marker':
            clips[(f[1], f[2], f[3])]['markers'].append(float(f[4]))
        elif f[0] == 'frame':
            frames.setdefault((f[1], f[2], f[3]), []).append(
                (float(f[5]), [float(v) for v in f[6:10]], None if f[10] == '-' else [float(v) for v in f[10:14]]))
    return units, clips, frames


def index_at(frames: list, time_ms: float) -> int:
    best = 0
    for i, (t, _, _) in enumerate(frames):
        if t <= time_ms + 1e-3:
            best = i
    return best


def pick(unit: str, facing: str, column: str, clips: dict, frames: dict) -> tuple[str, int] | None:
    def key(clip):
        return (unit, facing, clip)
    if column == 'idle':
        return 'idle', 0
    if column == 'move':
        clip = 'walk' if key('walk') in clips else 'run'
        return clip, len(frames[key(clip)]) // 2
    if column.startswith('attack'):
        c = clips[key('attack')]
        part = column.split(':')[1]
        if part == 'start':
            return 'attack', 0
        if part == 'marker':
            return 'attack', index_at(frames[key('attack')], c['markers'][0] if c['markers'] else 0)
        return 'attack', len(frames[key('attack')]) - 1
    if column == 'skill:marker':
        skills = sorted(k[2] for k in clips if k[:2] == (unit, facing) and k[2].startswith('skill_'))
        if not skills:
            return None
        c = clips[key(skills[0])]
        return skills[0], index_at(frames[key(skills[0])], c['markers'][0] if c['markers'] else 0)
    if column == 'hit':
        return 'hit', len(frames[key('hit')]) // 2
    return 'death', len(frames[key('death')]) - 1


def crop(cache: dict, pack_root: Path, files: dict, regions: dict, resource: str) -> Image.Image | None:
    if resource not in regions:
        return None
    name, region = regions[resource]
    if name not in cache:
        cache[name] = Image.open(pack_root / files[name]['path']).convert('RGBA')
    image = cache[name]
    if region is None:
        return image
    return image.crop((region['x'], region['y'], region['x'] + region['width'], region['y'] + region['height']))


def sheet(age: str, root: Path, out: Path) -> Path:
    files, regions, base = load_pack(root, age)
    clips_name, _ = regions['meta/clips']
    units, clips, frames = parse_clips((base / files[clips_name]['path']).read_text())
    backdrop_name, _ = regions['scene/backdrop']
    backdrop = Image.open(base / files[backdrop_name]['path']).convert('RGBA')
    strip = backdrop.resize((1920, 698)).crop((0, 490 - 120, CELL[0], 490 + 30))
    cache = {}
    label_w = 170
    canvas = Image.new('RGBA', (label_w + CELL[0] * len(COLUMNS), 24 + CELL[1] * len(units) * 2), (30, 30, 30, 255))
    draw = ImageDraw.Draw(canvas)
    for c, column in enumerate(COLUMNS):
        draw.text((label_w + c * CELL[0] + 4, 6), column, fill=(235, 235, 235, 255))
    row = 0
    for unit in units:
        for facing in ('right', 'left'):
            draw.text((6, 24 + row * CELL[1] + 60), f'{unit}\n{facing}', fill=(235, 235, 235, 255))
            for c, column in enumerate(COLUMNS):
                x0, y0 = label_w + c * CELL[0], 24 + row * CELL[1]
                cell = strip.copy()
                chosen = pick(unit, facing, column, clips, frames)
                if chosen:
                    clip, i = chosen
                    _, body, mask = frames[(unit, facing, clip)][i]
                    ground = (CELL[0] // 2, 120)
                    resource = f'{unit}/{facing}/{clip}/{i:03d}'
                    color = crop(cache, base, files, regions, f'unit/{resource}')
                    size = (max(1, round(body[2])), max(1, round(body[3])))
                    cell.alpha_composite(color.resize(size, Image.Resampling.LANCZOS),
                                         (round(ground[0] + body[0]), round(ground[1] + body[1])))
                    if mask:
                        paint = crop(cache, base, files, regions, f'mask/{resource}')
                        msize = (max(1, round(mask[2])), max(1, round(mask[3])))
                        shade = paint.resize(msize, Image.Resampling.LANCZOS)
                        rgb = ImageChops.multiply(shade.convert('RGB'), Image.new('RGB', msize, TEAM[facing]))
                        tinted = Image.merge('RGBA', (*rgb.split(), shade.getchannel('A').point(lambda v: v * 9 // 10)))
                        cell.alpha_composite(tinted, (round(ground[0] + mask[0]), round(ground[1] + mask[1])))
                    draw_cell = ImageDraw.Draw(cell)
                    draw_cell.text((4, CELL[1] - 14), f'{clip}#{i}', fill=(20, 20, 20, 255))
                canvas.alpha_composite(cell, (x0, y0))
            row += 1
    path = out / f'poses-{age}.png'
    canvas.convert('RGB').save(path, optimize=True)
    big = canvas.resize((canvas.width * 2, canvas.height * 2), Image.Resampling.NEAREST)
    big.convert('RGB').save(out / f'poses-{age}-2x.png', optimize=True)
    return path


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--packs', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    for age in AGES:
        print(sheet(age, args.packs, args.out))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
