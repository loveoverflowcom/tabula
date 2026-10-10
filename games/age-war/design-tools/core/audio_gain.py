"""Constant-gain offline review audio. Measurement is not listening acceptance."""
from __future__ import annotations
import argparse
import json
import math
from pathlib import Path
import re
import subprocess


def preview_gain_db(integrated_lufs: float, true_peak_dbfs: float,
                    target_lufs: float = -24.5, peak_limit_dbfs: float = -2.0) -> float:
    if not all(math.isfinite(x) for x in
               (integrated_lufs, true_peak_dbfs, target_lufs, peak_limit_dbfs)):
        raise ValueError('Nonfinite loudness measurement or target')
    return min(target_lufs - integrated_lufs, peak_limit_dbfs - true_peak_dbfs) - .1


def measure(path: Path) -> dict:
    result = subprocess.run(
        ['ffmpeg', '-hide_banner', '-i', str(path), '-af',
         'loudnorm=I=-24.5:TP=-2:LRA=10:print_format=json', '-f', 'null', '-'],
        capture_output=True, text=True, check=True)
    matches = re.findall(r'\{\s*"input_i".*?\}', result.stderr, re.S)
    if len(matches) != 1:
        raise ValueError('Expected one FFmpeg loudness measurement')
    data = json.loads(matches[0])
    return {'integrated_lufs': float(data['input_i']),
            'true_peak_dbfs': float(data['input_tp'])}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('input', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    if args.output.exists() or args.input.resolve() == args.output.resolve():
        parser.error('Choose a new output file; source audio is never overwritten.')
    before = measure(args.input)
    gain = preview_gain_db(before['integrated_lufs'], before['true_peak_dbfs'])
    subprocess.run(['ffmpeg', '-v', 'error', '-n', '-i', str(args.input), '-af',
                    f'volume={gain:.3f}dB', '-c:a', 'pcm_s16le', str(args.output)], check=True)
    after = measure(args.output)
    if after['true_peak_dbfs'] > -2.0:
        raise ValueError('Measured output exceeds review true-peak headroom')
    print(json.dumps({'gain_db': gain, 'before': before, 'after': after,
                      'method': 'constant gain; no compression', 'audition': 'NOT_RUN'}))


if __name__ == '__main__':
    main()
