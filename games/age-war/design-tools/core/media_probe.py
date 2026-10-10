"""Offline codec, frame-count and decode checks, without a playback or FPS claim."""
from __future__ import annotations
import argparse
import array
import hashlib
import json
import math
from pathlib import Path
import subprocess
import sys


def stream_checks(streams, width, height, duration, frames):
    video = next(s for s in streams if s['codec_type'] == 'video')
    audio = next(s for s in streams if s['codec_type'] == 'audio')
    return {
        'video_h264': video['codec_name'] == 'h264',
        'audio_aac_48k_stereo': audio['codec_name'] == 'aac' and int(audio['sample_rate']) == 48000 and int(audio['channels']) == 2,
        'dimensions': (video['width'], video['height']) == (width, height),
        'encoded_60fps': video['r_frame_rate'] == '60/1',
        'frame_count': int(video['nb_frames']) == frames,
        'video_duration': abs(float(video['duration']) - duration) <= 1 / 60 + .001,
        'audio_duration': abs(float(audio['duration']) - duration) <= .025,
    }


def inspect(path, width, height, duration, frames):
    probe = json.loads(subprocess.check_output(['ffprobe', '-v', 'error', '-show_streams', '-of', 'json', str(path)]))
    checks = stream_checks(probe['streams'], width, height, duration, frames)
    decoded = subprocess.run(['ffmpeg', '-v', 'error', '-threads', '1', '-i', str(path), '-f', 'null', '-'], capture_output=True)
    raw = subprocess.check_output(['ffmpeg', '-v', 'error', '-i', str(path), '-vn', '-ac', '2', '-ar', '48000', '-f', 'f32le', '-'])
    samples = array.array('f')
    samples.frombytes(raw)
    if sys.byteorder != 'little':
        samples.byteswap()
    finite = bool(samples) and all(math.isfinite(v) for v in samples)
    peak = max((abs(v) for v in samples), default=0)
    rms = math.sqrt(sum(v * v for v in samples) / len(samples)) if samples else 0
    checks.update(full_decode_no_errors=decoded.returncode == 0 and not decoded.stderr.strip(),
                  decoded_audio_finite=finite, decoded_audio_non_silent=rms > .001,
                  decoded_audio_non_clipping=peak < .98)
    return {'bytes': path.stat().st_size, 'local_sha256': hashlib.sha256(path.read_bytes()).hexdigest(),
            'checks': checks, 'failure_count': sum(not ok for ok in checks.values()),
            'scope': 'codec, decoding and numerical audio only',
            'audio_audition': 'NOT_RUN', 'browser_playback': 'NOT_RUN',
            'realtime_device_fps': 'NOT_RUN', 'visual_acceptance': 'NOT_RUN'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('input', type=Path)
    for name in ('width', 'height', 'frames'):
        parser.add_argument('--' + name, type=int, required=True)
    parser.add_argument('--duration', type=float, required=True)
    args = parser.parse_args()
    if min(args.width, args.height, args.frames, args.duration) <= 0 or not math.isfinite(args.duration):
        parser.error('Expected positive finite dimensions, duration and frame count.')
    report = inspect(args.input, args.width, args.height, args.duration, args.frames)
    print(json.dumps(report, indent=2))
    return bool(report['failure_count'])


if __name__ == '__main__':
    raise SystemExit(main())
