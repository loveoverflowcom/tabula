"""Pinned pointer geometry only, never browser, screenshot or durable evidence."""
import csv
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest import mock

from browser_acceptance import board_square, move


FIXTURE = Path(__file__).with_name("chess_board_layout_fixture.csv")
BOARD_VIEWPORTS = ((1100, 794), (1440, 904), (390, 788), (320, 584), (844, 334))


def fixture_rows():
    # These dimensions are already the game board viewport. The online caller
    # reserves its Rust status dock; standalone uses the complete playing canvas.
    with FIXTURE.open(newline="", encoding="utf-8") as source:
        return [{key: float(value) for key, value in row.items()}
                for row in csv.DictReader(source)]


class BoardSquareTests(unittest.TestCase):
    def test_shared_fixture_covers_exact_required_board_viewports(self):
        rows = fixture_rows()
        self.assertEqual([(row["board_viewport_width"], row["board_viewport_height"])
                          for row in rows], list(BOARD_VIEWPORTS))
        self.assertTrue(all(row["board_side"] > 0 for row in rows))

    def test_pinned_bounds_and_all_64_centers_in_both_orientations(self):
        checked = 0
        for row in fixture_rows():
            width, height = row["board_viewport_width"], row["board_viewport_height"]
            left, top, side = row["board_left"], row["board_top"], row["board_side"]
            cell = side / 8
            for flipped in (False, True):
                centers = []
                with self.subTest(board_viewport=(width, height), flipped=flipped):
                    for rank in range(8):
                        for file in range(8):
                            name = chr(ord("a") + file) + str(rank + 1)
                            x, y = board_square(width, height, name, flipped)
                            # A fixed board bounds fixture pins geometry independently
                            # of the helper's responsive layout calculation.
                            column, screen_row = (7 - file, rank) if flipped else (file, 7 - rank)
                            self.assertAlmostEqual(x, left + (column + .5) * cell, delta=1e-9)
                            self.assertAlmostEqual(y, top + (screen_row + .5) * cell, delta=1e-9)
                            self.assertTrue(left < x < left + side and top < y < top + side)
                            centers.append((x, y))
                            checked += 1
                    self.assertEqual(len(set(centers)), 64)
                    self.assertAlmostEqual(min(x for x, _ in centers) - cell / 2, left, delta=1e-9)
                    self.assertAlmostEqual(min(y for _, y in centers) - cell / 2, top, delta=1e-9)
                    self.assertAlmostEqual(max(x for x, _ in centers) + cell / 2, left + side, delta=1e-9)
                    self.assertAlmostEqual(max(y for _, y in centers) + cell / 2, top + side, delta=1e-9)
        self.assertEqual(checked, 640)

    def test_online_move_reserves_status_dock_once_at_the_caller(self):
        page = mock.MagicMock()
        canvas = page.locator.return_value
        canvas.bounding_box.return_value = {"width": 1100, "height": 850}
        response = mock.Mock(status=200)
        response.request.post_data = "original-command-bytes"
        page.expect_response.return_value.__enter__.return_value = SimpleNamespace(value=response)
        with mock.patch("browser_acceptance.board_square", wraps=board_square) as locate, \
                mock.patch("browser_acceptance.actual_response_json", return_value={"frames": [{"body": {"Ack": {}}}]}):
            self.assertEqual(move(page, "f2", "f3", False, "a" * 32), "original-command-bytes")
        self.assertEqual(locate.call_args_list, [mock.call(1100, 794, "f2", False),
                                               mock.call(1100, 794, "f3", False)])
        # These pinned centers use 794 directly. Subtracting another footer inside
        # the helper would change the board size and fail this check.
        self.assertEqual(canvas.click.call_count, 2)
        for call, expected in zip(canvas.click.call_args_list,
                                  ((520.7525, 547.5475), (520.7525, 481.7525))):
            self.assertEqual(call.kwargs["delay"], 70)
            self.assertAlmostEqual(call.kwargs["position"]["x"], expected[0], delta=1e-9)
            self.assertAlmostEqual(call.kwargs["position"]["y"], expected[1], delta=1e-9)


if __name__ == "__main__":
    unittest.main()
