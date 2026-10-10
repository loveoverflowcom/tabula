"""Stage the D06 private HTML review bundle after Rust verifies the source packs.

Public code remains in design-tools; all atlas pages, crops and delivery metadata
are written only to an ignored output tree. ZIP is a private review deliverable,
not an asset license or publication approval. No HTTP service is started here.
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import shutil
import subprocess
import sys
import zipfile

from PIL import Image

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[3]


def require_private_output(out: Path) -> None:
    out = out.resolve()
    try:
        relative = out.relative_to(REPO)
    except ValueError:
        return  # explicit external private destination
    result = subprocess.run(['git', 'check-ignore', '--quiet', str(relative)], cwd=REPO, check=False)
    if result.returncode != 0:
        raise ValueError('Private review output must be outside Git or ignored (use verification/age-war-d06).')


def verify_exports(site: Path, assets: dict) -> dict:
    """Check the non-empty exported domain, bounds and ordered timing."""
    frames, clips, pages = 0, 0, 0
    for age in assets['ages'].values():
        motion = age['motion']
        dimensions = {}
        for name, file in motion['files'].items():
            path = site / 'assets' / name
            if path.stat().st_size != file['bytes']:
                raise ValueError(f'Export byte count mismatch: {name}')
            with Image.open(path) as image:
                dimensions[name] = image.size
            pages += 1
        if not motion['clips'] or not motion['vfx']:
            raise ValueError('Empty motion export')
        for clip in motion['clips'].values():
            times = [frame['t'] for frame in clip['frames']]
            if not times or times != sorted(times) or times[0] != 0 or times[-1] > clip['duration'] + .01:
                raise ValueError('Invalid or empty clip timeline')
            clips += 1
            for frame in clip['frames']:
                rect = frame['sprite']['rect']
                w, h = dimensions[frame['sprite']['file']]
                if not rect or min(rect['width'], rect['height']) <= 0 or min(rect['x'], rect['y']) < 0 or rect['x'] + rect['width'] > w or rect['y'] + rect['height'] > h:
                    raise ValueError('Exported frame leaves decoded page')
                frames += 1
    return {'clips': clips, 'frames': frames, 'pages': pages, 'status': 'PASS'}


def stage(packs: Path, vfx_zip: Path, out: Path, binary: Path) -> dict:
    require_private_output(out)
    out.mkdir(parents=True, exist_ok=True)
    # Fail before copying any art if binding, integrity or the D01 oracle fails.
    subprocess.run([str(binary.resolve()), 'check', '--packs', str(packs.resolve()),
                    '--report', str((out / 'pack-check.json').resolve())], cwd=REPO, check=True,
                   stdout=subprocess.DEVNULL)
    site = out / 'site'
    subprocess.run([sys.executable, str(HERE / 'export_preview_assets.py'), '--packs', str(packs.resolve()),
                    '--vfx-zip', str(vfx_zip.resolve()), '--out', str((site / 'assets').resolve())], check=True)
    for name in ['index.html', 'main.mjs', 'layout.mjs', 'storyboard.mjs', 'states.mjs', 'hud.css', 'catalog.generated.json']:
        shutil.copyfile(HERE / name, site / name)
    shutil.copyfile(REPO / 'apps/web/style/tokens.css', site / 'tokens.css')
    shutil.copyfile(REPO / 'assets/brand/tabula-mark-primary.svg', site / 'tabula-mark.svg')
    (site / 'REVIEW.txt').write_text(
        'Age War D06 · PRIVATE design preview, not a game or native application.\n'
        'Run: python3 -m http.server 8124 --bind 127.0.0.1 --directory site\n'
        'Open http://127.0.0.1:8124/?age=Arcane&size=844x390\n'
        'Motion uses complete D05 density-1 frames. Buttons log proposed intents only.\n'
        'All numeric resources, queues, HP and outcomes are labelled review fixtures.\n'
        'Missing: base art, complete turret art, production HUD icons, SourceQA.\n'
        'Owner art/motion/audio/rights and authority decision PENDING. C01 BLOCKED.\n'
        'Do not redistribute artwork until rights are approved. See public D06 contract.\n')
    catalog = json.loads((site / 'catalog.generated.json').read_text())
    assets = json.loads((site / 'assets/assets.json').read_text())
    for age in catalog['ages']:
        if set(u['id'] for u in age['units']) != set(assets['ages'][age['age']]['units']):
            raise ValueError(f'Roster differs for {age["age"]}')
    exports = verify_exports(site, assets)
    subprocess.run([sys.executable, str(HERE / 'render_review.py'), '--site', str(site.resolve()),
                    '--out', str((site / 'review-compositions').resolve())], check=True)
    report = {
        'kind': 'private-design-review-bundle', 'rules': catalog['source'], 'packs': '0.5.0',
        'ages': len(catalog['ages']), 'units': sum(len(a['units']) for a in catalog['ages']),
        'source_verification': 'pack-check.json (Rust binding + size/BLAKE3 + compiled D01 timing oracle)',
        'asset_manifest': 'site/assets/assets.json (pack/resource/regions/logical box + full source-page BLAKE3)',
        'rights': 'PENDING owner review; no redistribution approval', 'code_gate': 'PENDING / C01 BLOCKED',
        'missing': ['SourceQA archive', 'D03 base states', 'production unit/spell icons', 'complete turret states'],
        'browser_acceptance': 'NOT_RUN by staging; record actual browser evidence separately',
        'exports': exports, 'png_review': '72 Pillow reference compositions + six-age sheet; NOT browser screenshots',
    }
    report['site_bytes'] = sum(p.stat().st_size for p in site.rglob('*') if p.is_file())
    archive = out / 'age-war-d06-private-preview.zip'
    with zipfile.ZipFile(archive, 'w', compression=zipfile.ZIP_DEFLATED, compresslevel=6) as bundle:
        for path in sorted(site.rglob('*')):
            if path.is_file():
                bundle.write(path, str(Path('site') / path.relative_to(site)))
    with zipfile.ZipFile(archive) as bundle:
        if bundle.testzip() is not None:
            raise ValueError('ZIP CRC failed')
        report['zip_members'] = len(bundle.namelist())
    report['zip_bytes'] = archive.stat().st_size
    (out / 'handoff.json').write_text(json.dumps(report, indent=2) + '\n')
    return report


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--packs', type=Path, default=REPO / 'target/age-war-d05/packs')
    parser.add_argument('--binary', type=Path, default=REPO / 'target/release/examples/age_war_d05_pilot')
    parser.add_argument('--vfx-zip', type=Path, required=True)
    parser.add_argument('--out', type=Path, default=REPO / 'verification/age-war-d06')
    args = parser.parse_args()
    print(json.dumps(stage(args.packs, args.vfx_zip, args.out, args.binary), indent=2))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
