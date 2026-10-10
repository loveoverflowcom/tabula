"""Numeric audio checks for the D05 runtime packs. Not a listening review.

Reads the private audio archive and the pilot's cue logs, then reports:
- WAV integrity against the delivery index (format, sample count, peaks);
- loop seams of every looping asset (boundary jump vs. typical step);
- OGG runtime encodes decoded by FFmpeg against their WAV reference length;
- the public scheduler run over real crowd cue logs (voice caps and drops);
- cue streams under full/low/reduced effects (effects never change audio cues).

`audition` stays NOT_RUN: no person listened through this tool.
"""
from __future__ import annotations

import argparse
import io
import json
from pathlib import Path
import subprocess
import sys
import wave
import zipfile

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'core'))
import audio_scheduler  # noqa: E402


def pcm(data: bytes) -> tuple[np.ndarray, int, int]:
    with wave.open(io.BytesIO(data)) as reader:
        channels, rate, width = reader.getnchannels(), reader.getframerate(), reader.getsampwidth()
        if width != 2:
            raise ValueError('expected 16-bit PCM')
        samples = np.frombuffer(reader.readframes(reader.getnframes()), dtype='<i2').astype(np.float64) / 32768
    return samples.reshape(-1, channels), rate, channels


# A seam may be as large as an ordinary loud step, but not a click above it.
SEAM_LIMIT = 1.25


def loop_seam(samples: np.ndarray, start: int, end: int) -> dict:
    """Jump across end->start compared with ordinary adjacent-sample steps."""
    body = samples[start:end]
    steps = np.abs(np.diff(body, axis=0))
    jump = np.abs(body[0] - body[-1])
    typical = np.percentile(steps, 99, axis=0)
    ratio = float(np.max(jump / np.maximum(typical, 1e-9)))
    return {'jump': round(float(np.max(jump)), 6), 'p99_step': round(float(np.max(typical)), 6),
            'ratio_to_p99_step': round(ratio, 3), 'pass': ratio <= SEAM_LIMIT}


def ogg_samples(data: bytes) -> int:
    raw = subprocess.run(['ffmpeg', '-v', 'error', '-i', 'pipe:0', '-f', 's16le', '-ac', '1', '-ar', '48000', '-'],
                         input=data, capture_output=True, check=True).stdout
    return len(raw) // 2


def scheduler_index(index: list) -> dict:
    out = {}
    for asset in index:
        out[asset['id']] = {'id': asset['id'], 'priority': asset['priority'], 'group': asset['group'],
                            'duration_ms': asset['duration_ms']}
    return out


def cue_audio(kind: str, recipe: str, index: dict) -> str | None:
    """Maps a pilot cue to the first delivered variation of its family."""
    prefix = {'attack': f'{recipe}_v', 'hit': f'{recipe}_v', 'skill': f'{recipe}_contact_v',
              'spell': f'{recipe}_contact_v'}[kind]
    candidates = sorted(i for i in index if i.startswith(prefix))
    return candidates[0] if candidates else None


def schedule_log(log: Path, index: dict) -> dict:
    events = []
    for line in log.read_text().splitlines():
        if line.startswith('#') or not line:
            continue
        t, kind, recipe, unit, team = line.split('\t')
        audio = cue_audio(kind, recipe, index)
        if audio:
            events.append({'presentation_ms': float(t), 'audio_id': audio, 'entity_id': unit, 'team': team})
    accepted, info = audio_scheduler.schedule(events, index)
    info.pop('drops')
    return {'cue_events': len(events), **info}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--audio-zip', type=Path, required=True)
    parser.add_argument('--pilot', type=Path, required=True, help='pilot matrix output directory')
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    archive = zipfile.ZipFile(args.audio_zip)
    index = json.loads(archive.read('manifest/audio-index-v01.json'))
    report = {'assets': len(index), 'wav': {'pass': 0, 'fail': []}, 'loops': {}, 'ogg': {'pass': 0, 'fail': []}}
    for asset in index:
        samples, rate, channels = pcm(archive.read(asset['wav']))
        peak = float(np.max(np.abs(samples))) if samples.size else 0.0
        ok = (rate == asset['sample_rate'] and channels == asset['channels']
              and len(samples) == asset['sample_count'] and 0 < peak < 0.98
              and abs(float(np.mean(samples))) < 0.01)
        report['wav']['pass' if ok else 'fail'] = report['wav']['pass'] + 1 if ok else \
            report['wav']['fail'] + [asset['id']]
        if asset['loop']:
            report['loops'][asset['id']] = loop_seam(samples, asset['loop_start_sample'],
                                                     asset['loop_end_sample_exclusive'])
        decoded = ogg_samples(archive.read(asset['ogg']))
        difference_ms = (decoded - asset['sample_count'] * 48000 / asset['sample_rate']) / 48
        entry = {'id': asset['id'], 'ogg_minus_wav_ms': round(difference_ms, 2)}
        if abs(difference_ms) <= 25:
            report['ogg']['pass'] += 1
        else:
            report['ogg']['fail'].append(entry)
        if asset['loop'] and decoded != asset['sample_count']:
            report.setdefault('ogg_loop_length_mismatch', []).append(entry)
    loops = report['loops'].values()
    report['loop_summary'] = {'count': len(report['loops']), 'pass': sum(v['pass'] for v in loops),
                              'worst_ratio': max(v['ratio_to_p99_step'] for v in loops)}
    sched = scheduler_index(index)
    report['scheduler'] = {}
    for run in ('crowd-48-full', 'crowd-100-full', 'lineup-Future', 'audio-silent'):
        log = args.pilot / run / 'cue-log.tsv'
        if log.exists():
            report['scheduler'][run] = schedule_log(log, sched)
    effects = {}
    for mode in ('full', 'low', 'reduced'):
        path = args.pilot / f'effects-{mode}' / 'report.json'
        if path.exists():
            effects[mode] = json.loads(path.read_text())['runs'][0]['play']['cues']
    report['effects_cue_streams'] = {'counts': effects,
                                     'identical': len(effects) == 3 and len({json.dumps(v, sort_keys=True)
                                                                              for v in effects.values()}) == 1}
    report['audition'] = 'NOT_RUN: no person listened; numeric checks only'
    args.out.write_text(json.dumps(report, indent=1) + '\n')
    print(json.dumps({'wav_pass': report['wav']['pass'], 'wav_fail': len(report['wav']['fail']),
                      'ogg_pass': report['ogg']['pass'], 'ogg_fail': len(report['ogg']['fail']),
                      'ogg_loop_length_mismatch': len(report.get('ogg_loop_length_mismatch', [])),
                      'loops': report['loop_summary'], 'scheduler': report['scheduler'],
                      'effects_identical': report['effects_cue_streams']['identical']}, indent=1))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
