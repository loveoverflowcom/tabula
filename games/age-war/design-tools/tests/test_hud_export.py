"""Synthetic checks for private-output protection and exported timing/pivots."""
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'hud-preview'))
from export_preview_assets import clips  # noqa: E402
from stage_preview import REPO, require_private_output  # noqa: E402


class HudExportTests(unittest.TestCase):
    def test_private_art_cannot_be_staged_under_public_source(self):
        with self.assertRaises(ValueError):
            require_private_output(REPO / 'games/age-war/design-tools/accidental-private-art')
        require_private_output(REPO / 'verification/age-war-d06-test')

    def test_export_preserves_exact_marker_times_and_logical_pivots(self):
        units, info = clips('unit\tu\nclip\tu\tright\tattack\t-\t1000\t0\t450\t2\n'
                            'marker\tu\tright\tattack\t450\tcontact\t-\n'
                            'frame\tu\tright\tattack\t0\t0\t-12\t-80\t48\t81\t-\n'
                            'frame\tu\tright\tattack\t1\t450\t-10\t-79\t50\t80\t-\n')
        key = ('u', 'right', 'attack')
        self.assertEqual(units, ['u'])
        self.assertEqual(info['definitions'][key], {'duration': 1000, 'loop': False})
        self.assertEqual(info['markers'][key], [450])
        self.assertEqual(info['frames'][key][1]['t'], 450)
        self.assertEqual(info['frames'][key][0]['box'], [-12, -80, 48, 81])


if __name__ == '__main__':
    unittest.main()
