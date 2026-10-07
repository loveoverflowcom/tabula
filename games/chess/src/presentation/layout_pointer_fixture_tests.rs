//! The browser pointer helper shares pinned board viewport geometry with the
//! actual presenter. These checks establish geometry, not rendered screenshots.

use super::{BoardLayout, PointerPosition, Square, Vec2, Viewport};

const FIXTURE: &str = include_str!("../../../../tests/online-match/chess_board_layout_fixture.csv");
const EXPECTED_VIEWPORTS: [(f32, f32); 5] = [
    (1100.0, 794.0),
    (1440.0, 904.0),
    (390.0, 788.0),
    (320.0, 584.0),
    (844.0, 334.0),
];

// Rust layout uses f32; the decimal fixture and Python helper use greater
// precision. This is less than one thousandth of a logical canvas pixel.
#[allow(clippy::float_arithmetic)]
fn assert_coordinate(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() <= 0.0002,
        "coordinate mismatch: {actual} != {expected}"
    );
}

#[test]
#[allow(clippy::float_arithmetic)]
fn browser_pointer_fixture_matches_board_layout_bounds_and_all_centers() {
    let mut lines = FIXTURE.lines();
    assert_eq!(
        lines.next(),
        Some("board_viewport_width,board_viewport_height,board_left,board_top,board_side")
    );
    let mut checked_viewports = Vec::new();
    let mut checked_centers = 0;
    for line in lines {
        let fields: Vec<f32> = line
            .split(',')
            .map(|value| value.parse().expect("fixture coordinate is valid"))
            .collect();
        let [width, height, left, top, side] = fields.as_slice() else {
            panic!("fixture row must have five fields");
        };
        checked_viewports.push((*width, *height));
        // Heights already describe the game board viewport. Online owns its
        // separate 56px status dock reservation; standalone has no subtraction.
        let viewport = Viewport::new(Vec2::new(*width, *height)).expect("valid fixture viewport");
        let cell = side / 8.0;
        for flipped in [false, true] {
            let layout = BoardLayout::oriented(viewport, flipped);
            let bounds = layout.board();
            assert_coordinate(bounds.origin().x, *left);
            assert_coordinate(bounds.origin().y, *top);
            assert_coordinate(bounds.size().x, *side);
            assert_coordinate(bounds.size().y, *side);
            assert_coordinate(layout.square_size(), cell);
            for value in 0..64 {
                let square = Square::new(value).expect("all 64 fixture squares are valid");
                let (file, row) = if flipped {
                    (7 - square.file(), square.rank())
                } else {
                    (square.file(), 7 - square.rank())
                };
                let expected = Vec2::new(
                    left + (f32::from(file) + 0.5) * cell,
                    top + (f32::from(row) + 0.5) * cell,
                );
                let rect = layout.square_rect(square).expect("square has a rectangle");
                let center = rect.origin() + rect.size() * 0.5;
                assert_coordinate(center.x, expected.x);
                assert_coordinate(center.y, expected.y);
                assert_eq!(
                    layout
                        .square_at(PointerPosition::new(expected).expect("finite fixture center")),
                    Some(square)
                );
                checked_centers += 1;
            }
        }
    }
    assert_eq!(checked_viewports, EXPECTED_VIEWPORTS);
    assert_eq!(checked_centers, 640);
}
