"""Synthetic tests for the private-runtime tooling. No delivered asset data."""
import hashlib
import io
import json
from pathlib import Path
import sys
import tempfile
import unittest
import zipfile

from PIL import Image

RUNTIME = Path(__file__).resolve().parents[1] / 'runtime'
sys.path.insert(0, str(RUNTIME))

import build_runtime_packs as builder  # noqa: E402
import packing  # noqa: E402
import verify_inputs  # noqa: E402


class PackingTests(unittest.TestCase):
    def test_shelf_pack_is_order_independent_and_non_overlapping(self):
        items = [((str(i),), 10 + i * 7 % 50, 20 + i * 13 % 70) for i in range(60)]
        first = packing.shelf_pack(items, page=256)
        second = packing.shelf_pack(list(reversed(items)), page=256)
        self.assertEqual(first, second)
        placements, pages = first
        boxes = {}
        for place in placements:
            cells = {(place.page, x, y) for x in range(place.x - packing.PAD, place.x + place.width + packing.PAD)
                     for y in range(place.y - packing.PAD, place.y + place.height + packing.PAD)}
            for other, taken in boxes.items():
                self.assertFalse(cells & taken, (place.key, other))
            boxes[place.key] = cells
            self.assertLessEqual(place.y + place.height + packing.PAD, pages[place.page][1])

    def test_oversized_frame_is_rejected(self):
        with self.assertRaises(ValueError):
            packing.shelf_pack([(('big',), 300, 10)], page=256)

    def test_extrusion_round_trip_and_tamper_detection(self):
        frame = Image.new('RGBA', (5, 3), (0, 0, 0, 0))
        frame.putpixel((0, 0), (255, 0, 0, 255))
        frame.putpixel((4, 2), (0, 255, 0, 200))
        page = Image.new('RGBA', (20, 20))
        packing.paste_extruded(page, frame, 6, 6)
        self.assertTrue(packing.extrusion_matches(page, 6, 6, 5, 3))
        page.putpixel((4, 4), (1, 2, 3, 4))
        self.assertFalse(packing.extrusion_matches(page, 6, 6, 5, 3))
        self.assertFalse(packing.extrusion_matches(page, 1, 1, 5, 3))

    def test_shaded_mask_never_paints_outside_the_colour_silhouette(self):
        colour = Image.new('RGBA', (4, 1), (200, 100, 50, 0))
        colour.putpixel((1, 0), (200, 100, 50, 128))
        colour.putpixel((2, 0), (200, 100, 50, 255))
        mask = Image.new('RGBA', (4, 1), (255, 255, 255, 255))
        alpha = list(packing.shaded_mask(colour, mask).getchannel('A').tobytes())
        self.assertEqual(alpha, [0, 128, 255, 0])

    def test_frames_are_never_upsampled(self):
        with self.assertRaises(ValueError):
            packing.scaled_size(10, 10, 1.5)
        self.assertEqual(packing.scaled_size(10, 3, 0.1), (1, 1))


def clip(times, duration, loop=False, markers=()):
    return {'clip': {'duration_ms': duration, 'loop': loop,
                     'markers': [{'time_ms': m, 'authority': False} for m in markers]},
            'frames': [{'time_ms': t} for t in times]}


class TimingAndFootTests(unittest.TestCase):
    def test_bake_grid_plus_exact_key_poses(self):
        good = clip([0, 41.667, 83.333, 100, 125, 150], 150, markers=[100])
        self.assertEqual(builder.check_clip_timing(good), [])
        self.assertIn('one-shot clip lacks its final pose at duration',
                      builder.check_clip_timing(clip([0, 41.667], 150)))
        self.assertTrue(any('no exact pose at marker' in issue for issue in
                            builder.check_clip_timing(clip([0, 41.667, 83.333], 83.333, markers=[50]))))
        self.assertIn('gap longer than one 24 Hz step', builder.check_clip_timing(clip([0, 60, 100], 100)))

    def test_planted_foot_speed_and_slide(self):
        frames = [{'time_ms': i * 50, 'sockets_original': {'feet': [[200 - i * 2, 228], [180, 220]]}}
                  for i in range(5)]
        speed, slide, pairs = builder.planted_foot_speed({'pivot': [192, 228], 'frames': frames})
        self.assertAlmostEqual(speed, 40.0)
        self.assertAlmostEqual(slide, 0.0)
        self.assertEqual(pairs, 4)
        frames[2]['sockets_original']['feet'][0][0] += 3
        _, slide, _ = builder.planted_foot_speed({'pivot': [192, 228], 'frames': frames})
        self.assertGreater(slide, 2.5)
        airborne = [{'time_ms': i * 50, 'sockets_original': {'feet': [[200, 200]]}} for i in range(3)]
        self.assertEqual(builder.planted_foot_speed({'pivot': [192, 228], 'frames': airborne})[2], 0)


class VerifyInputsTests(unittest.TestCase):
    def test_present_tampered_missing_and_sourceqa_are_distinct(self):
        with tempfile.TemporaryDirectory() as root:
            root = Path(root)
            member = b'payload'
            records = [{'path': 'data.bin', 'bytes': len(member), 'sha256': hashlib.sha256(member).hexdigest()}]
            buffer = io.BytesIO()
            with zipfile.ZipFile(buffer, 'w') as archive:
                archive.writestr('data.bin', member)
                archive.writestr('integrity/reconstructed-v05-members.json', json.dumps({'members': records}))
            good = buffer.getvalue()
            (root / 'good.zip').write_bytes(good)
            tampered = bytearray(good)
            tampered[len(good) // 2] ^= 0xFF
            (root / 'bad.zip').write_bytes(bytes(tampered))
            entry = lambda name, data: {'file_name': name, 'bytes': len(data),  # noqa: E731
                                        'sha256_local_payload': hashlib.sha256(data).hexdigest()}
            index = {'version': 't', 'current_files': [entry('good.zip', good), entry('bad.zip', good),
                                                       entry('absent.zip', good)],
                     'sourceqa': {'file_name': 'qa.zip', 'status': 'NO_VERIFIED_REMOTE_FILE', 'members_local': 1}}
            (root / 'index.json').write_text(json.dumps(index))
            report = verify_inputs.verify(root / 'index.json', root)
            status = {e['file_name']: e['status'] for e in report['entries']}
            self.assertEqual(status, {'good.zip': 'PASS', 'bad.zip': 'FAIL', 'absent.zip': 'NOT_RUN'})
            good_entry = next(e for e in report['entries'] if e['file_name'] == 'good.zip')
            self.assertEqual(good_entry['archive']['status'], 'PASS')
            self.assertEqual(report['sourceqa']['status'], 'MISSING')


if __name__ == '__main__':
    unittest.main()
