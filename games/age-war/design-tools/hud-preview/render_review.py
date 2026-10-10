"""Export static D06 art/layout compositions without a browser.

These are labelled reference compositions, NOT HTML screenshots, native renders
or browser font/accessibility evidence. Geometry comes from layout.mjs; art comes
from the private staged manifest. All palette values come from Tabula tokens.
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import re
import subprocess

from PIL import Image, ImageDraw, ImageFont

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[3]


def geometries() -> list:
    script = "import {layout,VIEWPORTS} from './layout.mjs'; console.log(JSON.stringify(VIEWPORTS.flatMap(v=>[1,1.3,2].map(font=>({name:v.name,font,geometry:layout({...v,font})})))));"
    return json.loads(subprocess.check_output(['node', '--input-type=module', '-e', script], cwd=HERE, text=True))


def palette() -> dict:
    # First :root block is the light theme. Do not define another colour source.
    css = (REPO / 'apps/web/style/tokens.css').read_text().split('}')[0]
    return dict(re.findall(r'--sys-color-([\w-]+):\s*([^;]+);', css))


def compose(site: Path, art: dict, age: dict, review: dict) -> Image.Image:
    g, colours = review['geometry'], palette()
    width, height = round(g['stage']['w']), round(g['inner']['y'] + g['inner']['h'] + (21 if review['name'] == '844x390' else 20 if review['name'] == '1024x768' else 0))
    image = Image.new('RGBA', (width, height), colours['surface'])
    assets = site / 'assets'
    draw = ImageDraw.Draw(image)
    font = ImageFont.truetype(str(REPO / 'assets/fonts/OpenSans-Regular.ttf'), round(12 * review['font']))
    small = ImageFont.truetype(str(REPO / 'assets/fonts/OpenSans-Regular.ttf'), 10)
    resources = art['ages'][age['age']]
    backdrop = Image.open(assets / resources['backdrop']['file']).convert('RGBA')
    image.alpha_composite(backdrop.resize((width, round(g['stage']['h'])), Image.Resampling.LANCZOS), (0, round(g['stage']['y'])))
    scale, ground = width / 1920, g['stage']['y'] + 490 * width / 1920
    for side, facing in enumerate(['right', 'left']):
        for index, unit in enumerate(age['units']):
            pose = resources['units'][unit['id']][facing]['attack']
            sprite = Image.open(assets / pose['file']).convert('RGBA')
            x = (280 + index * 118 if side == 0 else 1640 - index * 118) * scale
            bx, by, bw, bh = pose['box']
            sprite = sprite.resize((max(1, round(bw * scale)), max(1, round(bh * scale))), Image.Resampling.LANCZOS)
            image.alpha_composite(sprite, (round(x + bx * scale), round(ground + by * scale)))
            if side == 0:
                draw.ellipse((x - 10 * scale, ground, x + 10 * scale, ground + 6 * scale), outline=colours['team-1'], width=2)
            else:
                draw.polygon([(x, ground - 3 * scale), (x + 12 * scale, ground + 3 * scale),
                              (x, ground + 9 * scale), (x - 12 * scale, ground + 3 * scale)], outline=colours['team-2'], width=2)
    def panel(rect, text='', fill='container-high', text_font=font):
        x, y, w, h = (rect[k] for k in ['x', 'y', 'w', 'h'])
        draw.rounded_rectangle((x, y, x + w, y + h), radius=min(12, h / 4), fill=colours[fill], outline=colours['outline'])
        if text:
            draw.text((x + w / 2, y + h / 2), text, font=text_font, fill=colours['on-surface'], anchor='mm')
    hud = g['hud']
    panel(hud['top'], fill='surface'); panel(hud['dock'], fill='surface')
    age_label = ['I','II','III','IV','V','VI'][age['number'] - 1] if g['compact'] and review['font'] > 1 else age['label_vi']
    for region, text in [('hpLeft','● 2780'), ('hpRight','◆ 2460'), ('age',age_label), ('advance','U'), ('pause','P'), ('clock','04:32'), ('economy','680\n16/24\nXP280'), ('ageToggle','G')]:
        panel(hud[region], text)
    panel(hud['minimap'])
    for index, rect in enumerate(hud['cards']):
        panel(rect)
        unit = age['units'][index]
        icon = Image.open(assets / resources['units'][unit['id']]['icon']['file']).convert('RGBA')
        size = max(1, round(min(rect['w'] - 4, rect['h'] - 23)))
        icon.thumbnail((size, size), Image.Resampling.LANCZOS)
        image.alpha_composite(icon, (round(rect['x'] + (rect['w'] - icon.width) / 2), round(rect['y'] + 2)))
        draw.text((rect['x'] + rect['w'] / 2, rect['y'] + rect['h'] - 10), str(unit['cost']), font=font, fill=colours['on-surface'], anchor='mm')
    for index, rect in enumerate(g['queueEntries']):
        panel(rect, '◷ 120' if index == 0 else '·', text_font=small)
    for index, rect in enumerate(hud['sockets']): panel(rect, f'T{index + 1}')
    for index, rect in enumerate(hud['spells']): panel(rect, f'{"Q" if index == 0 else "E"}\n{age["spells"][index]["cost"]}')
    draw.text((g['inner']['x'] + 4, g['lane']['y'] + g['lane']['h'] + 10), 'REFERENCE COMPOSITION / NOT A BROWSER SCREENSHOT / BASE ART MISSING', font=small, fill=colours['on-surface'])
    return image.convert('RGB')


def render(site: Path, out: Path) -> dict:
    out.mkdir(parents=True, exist_ok=True)
    art = json.loads((site / 'assets/assets.json').read_text())
    catalog = json.loads((site / 'catalog.generated.json').read_text())
    reviews = geometries()
    images = []
    for age in catalog['ages']:
        for review in reviews:
            path = out / f'{age["age"]}-{review["name"]}-font{review["font"]}.png'
            result = compose(site, art, age, review)
            result.save(path, optimize=True)
            if review['name'] == '1280x720' and review['font'] == 1:
                images.append(result.resize((640, 360), Image.Resampling.LANCZOS))
    sheet = Image.new('RGB', (1280, 1080), palette()['surface'])
    for index, image in enumerate(images): sheet.paste(image, ((index % 2) * 640, (index // 2) * 360))
    sheet.save(out / 'six-ages-reference-composition.png')
    report = {'kind': 'reference-composition (Pillow), NOT browser/native screenshot', 'compositions': 72, 'ages': 6,
              'viewports': 4, 'fonts': [1, 1.3, 2], 'browser_acceptance': 'NOT_RUN', 'art_approval': 'PENDING'}
    (out / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    return report


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--site', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    # Do not allow private artwork to be written into a tracked source tree.
    from stage_preview import require_private_output
    require_private_output(args.out)
    print(json.dumps(render(args.site, args.out), indent=2))
