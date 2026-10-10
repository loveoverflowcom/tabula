"""Run the D05 presentation-pilot matrix and derive private evidence.

Prerequisites: sealed private packs (build_runtime_packs.py --seal) and a
release build of the pilot example:

    cargo build -p tabula-game-client --example age_war_d05_pilot --release

Windowed runs use `xvfb-run` (software GL); their frame times are not device
performance. Audio runs at volume 0 by default so automated runs stay silent;
`--audio-volume` is for an owner listening session.
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import subprocess
import sys

import numpy as np
from PIL import Image

AGES = ('Primitive', 'Ancient', 'Feudal', 'Arcane', 'Industrial', 'Future')
DT = 1000 / 60
# Multiples of 100/3 ms land on exact frames at 0.5x, 1x and 2x with DT.
SHARED_TIMES = [1200, 3000, 4800, 6000, 7200, 8400]
SEQUENCE_TIMES = list(range(400, 10401, 400))


def pilot(binary: Path, packs: Path, out: Path, name: str, args: list[str], window: bool = True) -> dict:
    run_dir = out / name
    run_dir.mkdir(parents=True, exist_ok=True)
    report = run_dir / 'report.json'
    command = [str(binary), 'check' if not window else 'run', '--packs', str(packs), '--report', str(report)]
    if window:
        command += ['--capture-dir', str(run_dir), '--fixed-dt', f'{DT:.6f}']
    command += args
    if window:
        command = ['xvfb-run', '-a', '-s', '-screen 0 2600x1300x24'] + command
    result = subprocess.run(command, capture_output=True, text=True, timeout=1800)
    (run_dir / 'stderr.txt').write_text(result.stderr[-20000:])
    data = json.loads(report.read_text()) if report.exists() else {}
    data['exit'] = result.returncode
    data['command'] = ' '.join(command[command.index(str(binary)):]) if str(binary) in command else ''
    return data


def frames_for(horizon_ms: float, speed: float) -> str:
    return str(int(horizon_ms / (DT * speed)) + 2)


def battlefield(path: Path) -> np.ndarray:
    image = np.asarray(Image.open(path).convert('RGB'))
    return image[40:]  # drop the run label strip


def compare_speeds(out: Path) -> dict:
    """Same presentation time must give the same pixels at every speed."""
    rows = {}
    for at in SHARED_TIMES:
        images = {}
        for speed in ('0.5', '1', '2'):
            matches = sorted((out / f'speed-{speed}').glob(f'*-t{at:05d}.png'))
            if matches:
                images[speed] = battlefield(matches[0])
        if len(images) == 3:
            base = images['1']
            rows[at] = {speed: int((np.abs(img.astype(int) - base.astype(int)) > 2).any(axis=2).sum())
                        for speed, img in images.items()}
        else:
            rows[at] = 'MISSING'
    ok = all(isinstance(v, dict) and all(n == 0 for n in v.values()) for v in rows.values())
    return {'differing_pixels_vs_1x': rows, 'status': 'PASS' if ok else 'FAIL'}


def contact_sheet(paths: list[Path], target: Path, columns: int, width: int) -> None:
    images = [Image.open(p).convert('RGB') for p in paths]
    if not images:
        return
    scale = width / images[0].width
    cell = (width, int(images[0].height * scale))
    rows = (len(images) + columns - 1) // columns
    sheet = Image.new('RGB', (cell[0] * columns, cell[1] * rows), (24, 24, 24))
    for index, image in enumerate(images):
        sheet.paste(image.resize(cell, Image.Resampling.LANCZOS),
                    ((index % columns) * cell[0], (index // columns) * cell[1]))
    sheet.save(target, optimize=True)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=Path('target/release/examples/age_war_d05_pilot'))
    parser.add_argument('--packs', type=Path, default=Path('target/age-war-d05/packs'))
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--only', default='', help='comma list of run groups')
    args = parser.parse_args()
    out = args.out
    out.mkdir(parents=True, exist_ok=True)
    only = set(filter(None, args.only.split(',')))
    want = lambda group: not only or group in only  # noqa: E731
    runs = {}
    seq = ','.join(map(str, SEQUENCE_TIMES))
    shared = ','.join(map(str, SHARED_TIMES))
    if want('check'):
        runs['check'] = pilot(args.binary, args.packs, out, 'check', [], window=False)
    if want('lineup'):
        for age in AGES:
            runs[f'lineup-{age}'] = pilot(args.binary, args.packs, out, f'lineup-{age}',
                                          ['--age', age, '--frames', frames_for(10500, 1), '--capture-at', seq])
    if want('speed'):
        for speed in ('0.5', '1', '2'):
            runs[f'speed-{speed}'] = pilot(args.binary, args.packs, out, f'speed-{speed}',
                                           ['--age', 'Future', '--speed', speed, '--frames', frames_for(9000, float(speed)),
                                            '--capture-at', shared])
        runs['speed-compare'] = compare_speeds(out)
    if want('effects'):
        for effects in ('full', 'low', 'reduced'):
            runs[f'effects-{effects}'] = pilot(args.binary, args.packs, out, f'effects-{effects}',
                                               ['--age', 'Future', '--effects', effects,
                                                '--frames', frames_for(9000, 1), '--capture-at', shared])
    if want('sizes'):
        for size in ('844x390', '1024x768', '1280x720', '1920x1080'):
            runs[f'size-{size}'] = pilot(args.binary, args.packs, out, f'size-{size}',
                                         ['--age', 'Arcane', '--size', size, '--frames', frames_for(6000, 1),
                                          '--capture-at', '2400,4400,5600'])
    if want('crowd'):
        # 24/side is the D01 population cap (single file); 50/side is a labelled stress.
        for per_side, effects in ((24, 'full'), (50, 'full'), (50, 'low'), (50, 'reduced')):
            runs[f'crowd-{per_side * 2}-{effects}'] = pilot(
                args.binary, args.packs, out, f'crowd-{per_side * 2}-{effects}',
                ['--age', 'Future', '--scenario', 'crowd', '--crowd', str(per_side), '--effects', effects,
                 '--frames', frames_for(6000, 1), '--capture-at', '3000,5000'])
    if want('density'):
        # The renderer selects density from DPI (density_for_dpi), as on device.
        # Xvfb has no high-DPI framebuffer, so these runs measure density
        # selection and residency only; their pixels are not a device render.
        runs['dpi2-960x540'] = pilot(args.binary, args.packs, out, 'dpi2-960x540',
                                     ['--age', 'Arcane', '--dpi', '2', '--size', '1920x1080',
                                      '--frames', frames_for(6000, 1)])
        runs['phone-844x390-dpi3'] = pilot(args.binary, args.packs, out, 'phone-844x390-dpi3',
                                           ['--age', 'Arcane', '--dpi', '3', '--size', '2532x1170',
                                            '--frames', frames_for(6000, 1)])
    if want('loadcycle'):
        for dpi in ('1', '2'):
            runs[f'loadcycle-dpi{dpi}'] = pilot(args.binary, args.packs, out, f'loadcycle-dpi{dpi}',
                                                ['--scenario', 'loadcycle', '--dpi', dpi, '--frames', '90'])
    if want('audio'):
        runs['audio-silent'] = pilot(args.binary, args.packs, out, 'audio-silent',
                                     ['--age', 'Arcane', '--audio', '--audio-volume', '0',
                                      '--frames', frames_for(9000, 1)])
    if want('lineup'):
        for age in AGES:
            caps = sorted((out / f'lineup-{age}').glob('*-t*.png'))
            contact_sheet(caps[::2], out / f'contact-lineup-{age}.png', 4, 640)
    if want('effects'):
        caps = [p for e in ('full', 'low', 'reduced') for p in sorted((out / f'effects-{e}').glob('*-t*.png'))[1:4]]
        contact_sheet(caps, out / 'contact-effects-full-low-reduced.png', 3, 640)
    if want('crowd'):
        caps = [p for name in ('crowd-48-full', 'crowd-100-full', 'crowd-100-low', 'crowd-100-reduced')
                for p in sorted((out / name).glob('*-t*.png'))]
        contact_sheet(caps, out / 'contact-crowd.png', 2, 960)
    (out / 'matrix.json').write_text(json.dumps(runs, indent=1) + '\n')
    failed = [name for name, run in runs.items() if isinstance(run, dict) and run.get('exit', 0) != 0
              or (isinstance(run, dict) and run.get('status') == 'FAIL')]
    print(json.dumps({'runs': len(runs), 'failed': failed}))
    return 1 if failed else 0


if __name__ == '__main__':
    sys.exit(main())
