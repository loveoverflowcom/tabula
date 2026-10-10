"""Synthetic codec metadata; no delivered asset data."""
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location('media_probe', Path(__file__).resolve().parents[1] / 'core/media_probe.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class ProbeTests(unittest.TestCase):
    def streams(self):
        return [{'codec_type': 'video', 'codec_name': 'h264', 'width': 80, 'height': 60,
                 'r_frame_rate': '60/1', 'nb_frames': '60', 'duration': '1'},
                {'codec_type': 'audio', 'codec_name': 'aac', 'sample_rate': '48000',
                 'channels': 2, 'duration': '1'}]

    def test_matching_nonempty_synthetic_streams(self):
        self.assertTrue(all(module.stream_checks(self.streams(), 80, 60, 1, 60).values()))

    def test_mismatch_is_not_reported_as_a_pass(self):
        streams = self.streams()
        streams[0]['nb_frames'] = '59'
        checks = module.stream_checks(streams, 80, 60, 1, 60)
        self.assertFalse(checks['frame_count'])

    def test_no_audio_does_not_produce_empty_success(self):
        with self.assertRaises(StopIteration):
            module.stream_checks(self.streams()[:1], 80, 60, 1, 60)


if __name__ == '__main__':
    unittest.main()
