//! What a feature is worth, and to whom.
//!
//! Pure functions over a [`Feature`] and the [`Board`] it sits on. They are the
//! **single authority** for every point in the game: the same three functions
//! score a feature that completes mid-match and one that is still unfinished
//! when the bag runs out, so the two cannot drift apart the way a
//! `score_completed` / `score_final` pair would.
//!
//! # The values
//!
//! | Feature | Completed | Unfinished, at end of game |
//! |---|---|---|
//! | Road | 1 per tile | 1 per tile |
//! | City | 2 per tile, +2 per pennant | 1 per tile, +1 per pennant |
//! | Monastery | 9 (its own tile and all eight neighbours) | 1 + however many neighbours it has |
//!
//! A monastery is why the per-tile shape is not enough on its own: a complete
//! monastery is worth nine points while covering one tile, so the arithmetic
//! lives here in full rather than as a `points_per_tile` constant that would be
//! right for two kinds out of three.
//!
//! # Majority, and ties
//!
//! Every seat with the **most** followers on a feature scores its full value.
//! Ties share rather than split: two seats with two followers each both take the
//! whole amount. A feature with no followers scores nothing and is still retired,
//! so it cannot be counted again at the end of the game.
//!
//! @ai.role domain-rule
//! @ai.domain tiles.rules.scoring
//! @ai.pure true
//! @ai.invariant every-majority-holder-scores-the-full-value
//! @ai.invariant an-unclaimed-feature-awards-nothing
//! @ai.evidence tests::a_completed_city_pays_two_a_tile_and_two_a_pennant
//! @ai.evidence tests::tied_majorities_both_score_the_full_value

#![allow(clippy::doc_markdown)]

use tabula_core::SeatId;

use super::feature::{Feature, MONASTERY_NEIGHBOURS};
use super::tile::{Board, FeatureKind};

/// Followers each seat starts with.
pub const MEEPLES_PER_SEAT: u8 = 7;

/// What a feature pays out, and to whom. Ascending by seat, so the award order
/// is canonical (I-2) and reproducible in an event stream.
pub type Awards = Vec<(SeatId, i64)>;

/// The value of `feature` if it is complete.
///
/// # Panics
/// Never. Every arm is total, and the arithmetic is saturating.
#[must_use]
pub fn completed_value(feature: &Feature) -> i64 {
    let tiles = i64::try_from(feature.tiles().len()).unwrap_or(i64::MAX);
    let pennants = i64::from(feature.pennants());
    match feature.kind() {
        FeatureKind::Road => tiles,
        FeatureKind::City => tiles.saturating_mul(2).saturating_add(pennants * 2),
        // Its own tile plus the eight that must surround it.
        FeatureKind::Monastery => 1 + i64::from(MONASTERY_NEIGHBOURS),
    }
}

/// The value of `feature` if the bag ran out before it was finished.
///
/// A monastery needs `board` because its partial value counts the neighbours it
/// actually acquired; the other two kinds count only their own tiles.
#[must_use]
pub fn incomplete_value(feature: &Feature, board: &Board) -> i64 {
    let tiles = i64::try_from(feature.tiles().len()).unwrap_or(i64::MAX);
    let pennants = i64::from(feature.pennants());
    match feature.kind() {
        FeatureKind::Road => tiles,
        FeatureKind::City => tiles.saturating_add(pennants),
        FeatureKind::Monastery => {
            let neighbours = feature
                .members()
                .iter()
                .map(|member| i64::from(board.surrounding_count(member.coord())))
                .max()
                .unwrap_or(0);
            1 + neighbours
        }
    }
}

/// The seats holding the most followers on `feature`, ascending. Empty when
/// nobody has claimed it.
#[must_use]
pub fn majority(feature: &Feature) -> Vec<SeatId> {
    let counts = feature.follower_counts();
    let Some(best) = counts.values().copied().max() else {
        return Vec::new();
    };
    counts
        .into_iter()
        .filter(|(_, count)| *count == best)
        .map(|(seat, _)| seat)
        .collect()
}

/// What `feature` pays out. `complete` selects which value table applies.
#[must_use]
pub fn awards(feature: &Feature, board: &Board, complete: bool) -> Awards {
    let value = if complete {
        completed_value(feature)
    } else {
        incomplete_value(feature, board)
    };
    majority(feature)
        .into_iter()
        .map(|seat| (seat, value))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::rules::coord::{Coord, Rotation};
    use crate::rules::feature::{FeatureGraph, SegmentRef};
    use crate::rules::tile::{PlacedTile, TileKind, START_TILE};

    fn kind_named(name: &str) -> TileKind {
        TileKind::all()
            .find(|kind| kind.def().name == name)
            .unwrap_or_else(|| panic!("no tile kind named {name}"))
    }

    /// Build a board and its graph together, as `rules::feature`'s tests do.
    fn build(tiles: &[(i16, i16, &str, Rotation)]) -> (Board, FeatureGraph) {
        let mut board = Board::new();
        let mut graph = FeatureGraph::new();
        for (x, y, name, rotation) in tiles {
            let coord = Coord::new(*x, *y).expect("test coordinate is in range");
            let kind = if *name == "start" {
                START_TILE
            } else {
                kind_named(name)
            };
            board.insert(coord, PlacedTile::new(kind, *rotation));
            graph.place_tile(&board, coord);
        }
        (board, graph)
    }

    fn feature_at(graph: &FeatureGraph, x: i16, y: i16, index: u8) -> &Feature {
        let segment = SegmentRef::new(Coord::new(x, y).unwrap(), index).unwrap();
        graph
            .feature(graph.of(segment).expect("the segment is on the board"))
            .expect("the id names a feature")
    }

    /// Three straight road tiles in a row, capped at both ends, is a complete
    /// three-tile road worth three.
    #[test]
    fn a_completed_road_pays_one_a_tile() {
        // `road-curve` is Road{North, West}; turned so one arm faces the row
        // and the other leaves the row, it caps the line.
        let (board, mut graph) = build(&[
            (0, 0, "road-straight", Rotation::R90),
            (1, 0, "road-straight", Rotation::R90),
            (2, 0, "road-straight", Rotation::R90),
        ]);
        let road = SegmentRef::new(Coord::ORIGIN, 0).unwrap();
        assert!(graph.place_follower(road, SeatId(1)));

        let feature = feature_at(&graph, 0, 0, 0);
        assert_eq!(feature.tiles().len(), 3);
        assert_eq!(completed_value(feature), 3);
        assert_eq!(awards(feature, &board, true), vec![(SeatId(1), 3)]);
        // Unfinished, the same road is worth the same: roads do not lose value.
        assert_eq!(incomplete_value(feature, &board), 3);
    }

    #[test]
    fn a_completed_city_pays_two_a_tile_and_two_a_pennant() {
        // Two caps facing each other close a two-tile city; swap one for the
        // pennant version to add a shield without changing the shape.
        let (board, mut graph) = build(&[
            (0, 0, "city-cap", Rotation::R0),
            (0, -1, "city-cap", Rotation::R180),
        ]);
        let city = SegmentRef::new(Coord::ORIGIN, 0).unwrap();
        assert!(graph.place_follower(city, SeatId(0)));

        let feature = feature_at(&graph, 0, 0, 0);
        assert!(feature.is_complete());
        assert_eq!(feature.tiles().len(), 2);
        assert_eq!(feature.pennants(), 0);
        assert_eq!(completed_value(feature), 4);
        assert_eq!(incomplete_value(feature, &board), 2);

        // Now with a pennant on the far tile.
        let (pennant_board, mut pennant_graph) = build(&[
            (0, 0, "city-cap", Rotation::R0),
            (0, -1, "city-corner-road-pennant", Rotation::R180),
        ]);
        assert!(pennant_graph.place_follower(city, SeatId(0)));
        let pennant_feature = feature_at(&pennant_graph, 0, 0, 0);
        assert_eq!(pennant_feature.pennants(), 1);
        assert_eq!(pennant_feature.tiles().len(), 2);
        // 2 tiles x 2 + 1 pennant x 2 = 6, complete or not by the same table.
        assert_eq!(completed_value(pennant_feature), 6);
        assert_eq!(incomplete_value(pennant_feature, &pennant_board), 3);
        assert_eq!(
            awards(pennant_feature, &pennant_board, false),
            vec![(SeatId(0), 3)]
        );
    }

    #[test]
    fn a_completed_monastery_pays_nine_and_an_unfinished_one_pays_what_it_has() {
        let mut board = Board::new();
        let mut graph = FeatureGraph::new();
        let centre = Coord::ORIGIN;
        board.insert(
            centre,
            PlacedTile::new(kind_named("monastery"), Rotation::R0),
        );
        graph.place_tile(&board, centre);
        let monastery = SegmentRef::new(centre, 0).unwrap();
        assert!(graph.place_follower(monastery, SeatId(2)));

        // Alone: worth one point at the end of the game.
        assert_eq!(incomplete_value(feature_at(&graph, 0, 0, 0), &board), 1);

        for (filled, neighbour) in centre.surrounding().enumerate() {
            board.insert(
                neighbour,
                PlacedTile::new(kind_named("monastery"), Rotation::R0),
            );
            graph.place_tile(&board, neighbour);
            let expected = 1 + i64::try_from(filled + 1).unwrap();
            assert_eq!(
                incomplete_value(feature_at(&graph, 0, 0, 0), &board),
                expected
            );
        }

        let feature = feature_at(&graph, 0, 0, 0);
        assert!(feature.is_complete());
        assert_eq!(completed_value(feature), 9);
        assert_eq!(
            incomplete_value(feature, &board),
            9,
            "a monastery that happens to be surrounded is worth the same either way"
        );
        assert_eq!(awards(feature, &board, true), vec![(SeatId(2), 9)]);
    }

    #[test]
    fn tied_majorities_both_score_the_full_value() {
        // A three-tile road claimed at both ends: after the middle tile merges
        // them, both seats hold one follower.
        let straight = kind_named("road-straight");
        let mut board = Board::new();
        let mut graph = FeatureGraph::new();
        for x in [-1i16, 1] {
            let coord = Coord::new(x, 0).unwrap();
            board.insert(coord, PlacedTile::new(straight, Rotation::R90));
            graph.place_tile(&board, coord);
        }
        assert!(graph.place_follower(
            SegmentRef::new(Coord::new(-1, 0).unwrap(), 0).unwrap(),
            SeatId(0)
        ));
        assert!(graph.place_follower(
            SegmentRef::new(Coord::new(1, 0).unwrap(), 0).unwrap(),
            SeatId(3)
        ));
        board.insert(Coord::ORIGIN, PlacedTile::new(straight, Rotation::R90));
        graph.place_tile(&board, Coord::ORIGIN);

        let feature = feature_at(&graph, 0, 0, 0);
        assert_eq!(
            feature.follower_counts(),
            BTreeMap::from([(SeatId(0), 1), (SeatId(3), 1)])
        );
        assert_eq!(majority(feature), vec![SeatId(0), SeatId(3)]);
        let value = incomplete_value(feature, &board);
        assert_eq!(value, 3);
        assert_eq!(
            awards(feature, &board, false),
            vec![(SeatId(0), value), (SeatId(3), value)],
            "a tie shares the rank, not the points"
        );
    }

    #[test]
    fn a_clear_majority_takes_it_all() {
        let straight = kind_named("road-straight");
        let mut board = Board::new();
        let mut graph = FeatureGraph::new();
        // Three separately-claimed road tiles, then two joins.
        for x in [-2i16, 0, 2] {
            let coord = Coord::new(x, 0).unwrap();
            board.insert(coord, PlacedTile::new(straight, Rotation::R90));
            graph.place_tile(&board, coord);
        }
        assert!(graph.place_follower(
            SegmentRef::new(Coord::new(-2, 0).unwrap(), 0).unwrap(),
            SeatId(0)
        ));
        assert!(graph.place_follower(SegmentRef::new(Coord::ORIGIN, 0).unwrap(), SeatId(0)));
        assert!(graph.place_follower(
            SegmentRef::new(Coord::new(2, 0).unwrap(), 0).unwrap(),
            SeatId(1)
        ));
        for x in [-1i16, 1] {
            let coord = Coord::new(x, 0).unwrap();
            board.insert(coord, PlacedTile::new(straight, Rotation::R90));
            graph.place_tile(&board, coord);
        }

        let feature = feature_at(&graph, 0, 0, 0);
        assert_eq!(feature.tiles().len(), 5);
        assert_eq!(majority(feature), vec![SeatId(0)]);
        assert_eq!(awards(feature, &board, false), vec![(SeatId(0), 5)]);
    }

    #[test]
    fn an_unclaimed_feature_awards_nothing_to_anybody() {
        let (board, graph) = build(&[(0, 0, "start", Rotation::R0)]);
        let feature = feature_at(&graph, 0, 0, 0);
        assert!(majority(feature).is_empty());
        assert!(awards(feature, &board, true).is_empty());
        assert!(awards(feature, &board, false).is_empty());
        // The value is still well defined; it just has no recipient.
        assert_eq!(completed_value(feature), 2);
    }
}
