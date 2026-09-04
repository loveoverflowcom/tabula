//! Placement legality — pure functions over a [`Board`].
//!
//! Nothing here touches `State`, `Ctx`, or a seat. That is deliberate: the same
//! functions answer the question for the rules (which must be authoritative),
//! for the bot (which sees only a `View`), and for the presenter (which
//! highlights targets). One implementation means those three can never
//! disagree, which is the same reason doc 02 §3.2 refuses a separate
//! `validate_command`.
//!
//! @ai.role domain-rule
//! @ai.domain tiles.rules.placement
//! @ai.pure true
//! @ai.invariant placement-requires-a-touching-neighbour
//! @ai.invariant placement-requires-matching-terrain-on-every-shared-edge
//! @ai.law legal-placements-agrees-with-is-legal-placement
//! @ai.evidence tests::legality_is_exhaustively_symmetric_over_every_kind_and_rotation
//! @ai.evidence crate::rules::placement::tests::enumeration_agrees_with_the_predicate

#![allow(clippy::doc_markdown)]

use super::coord::{Coord, Rotation, Side};
use super::tile::{Board, PlacedTile, TileKind};

/// Why a tile cannot go on a square.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PlacementError {
    #[error("that square already holds a tile")]
    Occupied,
    #[error("a tile must touch at least one tile already on the board")]
    NotAdjacent,
    #[error("the terrain on a shared edge does not match")]
    TerrainMismatch { side: Side },
}

/// Whether `tile` may be placed at `coord`.
///
/// Two conditions, in the order a player would check them: the square is free
/// and touches the board, and every shared edge shows the same terrain on both
/// sides.
///
/// # Errors
/// [`PlacementError`] naming which condition failed, so a rejection can say
/// something better than "illegal".
pub fn check_placement(
    board: &Board,
    coord: Coord,
    tile: PlacedTile,
) -> Result<(), PlacementError> {
    if board.contains(coord) {
        return Err(PlacementError::Occupied);
    }

    let mut touches = false;
    for (side, neighbour_coord) in coord.orthogonal() {
        let Some(neighbour) = board.get(neighbour_coord) else {
            continue;
        };
        touches = true;
        if tile.terrain(side) != neighbour.terrain(side.opposite()) {
            return Err(PlacementError::TerrainMismatch { side });
        }
    }

    if touches {
        Ok(())
    } else {
        Err(PlacementError::NotAdjacent)
    }
}

/// Convenience predicate over [`check_placement`].
#[must_use]
pub fn is_legal_placement(board: &Board, coord: Coord, tile: PlacedTile) -> bool {
    check_placement(board, coord, tile).is_ok()
}

/// Every free square orthogonally touching a placed tile, in canonical order.
///
/// This is the candidate set every placement question ranges over. On a board
/// of *n* tiles it holds at most 4*n* squares, so enumerating it and the four
/// rotations is bounded by `16n` legality checks — small enough that no caller
/// needs an index, and the absence of one is what keeps `Board` a plain map.
#[must_use]
pub fn frontier(board: &Board) -> Vec<Coord> {
    let mut candidates: Vec<Coord> = board
        .iter()
        .flat_map(|(coord, _)| coord.orthogonal().map(|(_, neighbour)| neighbour))
        .filter(|coord| !board.contains(*coord))
        .collect();
    candidates.sort_unstable();
    candidates.dedup();
    candidates
}

/// Every legal `(square, rotation)` for `kind`, grouped by square, in canonical
/// order and with the rotations of each square ascending.
///
/// Grouping is not a formatting choice: it is the shape
/// [`tabula_game_api::LegalCommands::Hints`] wants — one hint per highlightable
/// target — and it is why Tiles does not need `Enumerated` here.
#[must_use]
pub fn legal_placements(board: &Board, kind: TileKind) -> Vec<(Coord, Vec<Rotation>)> {
    frontier(board)
        .into_iter()
        .filter_map(|coord| {
            let rotations: Vec<Rotation> = Rotation::ALL
                .into_iter()
                .filter(|rotation| {
                    is_legal_placement(board, coord, PlacedTile::new(kind, *rotation))
                })
                .collect();
            (!rotations.is_empty()).then_some((coord, rotations))
        })
        .collect()
}

/// Whether `kind` can be placed anywhere at all.
///
/// A tile that cannot is discarded and redrawn — the rule that keeps a match
/// from deadlocking on an unplaceable draw.
#[must_use]
pub fn has_any_legal_placement(board: &Board, kind: TileKind) -> bool {
    frontier(board).into_iter().any(|coord| {
        Rotation::ALL
            .into_iter()
            .any(|rotation| is_legal_placement(board, coord, PlacedTile::new(kind, rotation)))
    })
}

/// The first legal `(square, rotation)` in canonical order.
///
/// Used only by the turn-deadline rule, which must resolve a stalled turn
/// **deterministically**: "first in canonical order" is a rule anyone can
/// reproduce, and every seat gets the same one.
#[must_use]
pub fn first_legal_placement(board: &Board, kind: TileKind) -> Option<(Coord, Rotation)> {
    legal_placements(board, kind)
        .into_iter()
        .next()
        .and_then(|(coord, rotations)| rotations.first().map(|rotation| (coord, *rotation)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::tile::{Terrain, START_TILE, TILE_SET};

    fn opening_board() -> Board {
        let mut board = Board::new();
        board.insert(
            Coord::ORIGIN,
            PlacedTile::new(START_TILE, Rotation::default()),
        );
        board
    }

    #[test]
    fn the_first_tile_has_nowhere_to_go_on_an_empty_board() {
        let board = Board::new();
        assert_eq!(
            check_placement(
                &board,
                Coord::ORIGIN,
                PlacedTile::new(START_TILE, Rotation::R0)
            ),
            Err(PlacementError::NotAdjacent)
        );
        assert!(frontier(&board).is_empty());
    }

    #[test]
    fn an_occupied_square_is_refused_before_anything_else_is_checked() {
        let board = opening_board();
        assert_eq!(
            check_placement(
                &board,
                Coord::ORIGIN,
                PlacedTile::new(START_TILE, Rotation::R0)
            ),
            Err(PlacementError::Occupied)
        );
    }

    #[test]
    fn a_square_touching_nothing_is_refused_however_well_it_would_match() {
        let board = opening_board();
        let far = Coord::new(5, 5).unwrap();
        assert_eq!(
            check_placement(&board, far, PlacedTile::new(START_TILE, Rotation::R0)),
            Err(PlacementError::NotAdjacent)
        );
    }

    #[test]
    fn a_mismatched_shared_edge_is_refused_and_names_the_side() {
        let board = opening_board();
        // The start tile shows City to its north, so a tile showing Field back
        // at it cannot go there. `city-cap` unrotated shows City north and
        // Field south, so its south edge faces the start tile's city.
        let north = Coord::ORIGIN.neighbour(Side::North).unwrap();
        let city_cap = TileKind::new(7).unwrap();
        assert_eq!(city_cap.def().name, "city-cap");
        assert_eq!(
            check_placement(&board, north, PlacedTile::new(city_cap, Rotation::R0)),
            Err(PlacementError::TerrainMismatch { side: Side::South })
        );
        // Turned half way round it shows its city south, into the start tile's.
        assert!(is_legal_placement(
            &board,
            north,
            PlacedTile::new(city_cap, Rotation::R180)
        ));
    }

    /// The core adjacency law, walked exhaustively rather than sampled: for
    /// every kind, every rotation, and every one of the start tile's four
    /// neighbours, legality must be exactly "the two facing terrains are
    /// equal". The oracle is written from the *definition* of adjacency and
    /// never calls `check_placement`.
    #[test]
    fn legality_is_exhaustively_symmetric_over_every_kind_and_rotation() {
        let board = opening_board();
        let start = PlacedTile::new(START_TILE, Rotation::R0);
        let mut checked = 0usize;

        for kind in TileKind::all() {
            for rotation in Rotation::ALL {
                let tile = PlacedTile::new(kind, rotation);
                for side in Side::ALL {
                    let coord = Coord::ORIGIN.neighbour(side).unwrap();
                    let facing_terrain = start.terrain(side);
                    let our_terrain = tile.terrain(side.opposite());
                    let expected = facing_terrain == our_terrain;
                    assert_eq!(
                        is_legal_placement(&board, coord, tile),
                        expected,
                        "{} at {rotation:?} on the {side:?} of the start tile",
                        kind.def().name
                    );
                    checked += 1;
                }
            }
        }

        assert_eq!(checked, TILE_SET.len() * 4 * 4);
    }

    /// Adjacency is a symmetric relation: if A may sit north of B, then B may
    /// sit south of A. A one-sided implementation would pass the test above and
    /// fail this one.
    #[test]
    fn adjacency_compatibility_is_symmetric_between_the_two_tiles() {
        for kind_a in TileKind::all() {
            for kind_b in TileKind::all() {
                for rotation in Rotation::ALL {
                    let a = PlacedTile::new(kind_a, rotation);
                    let b = PlacedTile::new(kind_b, Rotation::R0);

                    let mut with_a = Board::new();
                    with_a.insert(Coord::ORIGIN, a);
                    let north = Coord::ORIGIN.neighbour(Side::North).unwrap();
                    let b_above_a = is_legal_placement(&with_a, north, b);

                    let mut with_b = Board::new();
                    with_b.insert(Coord::ORIGIN, b);
                    let south = Coord::ORIGIN.neighbour(Side::South).unwrap();
                    let a_below_b = is_legal_placement(&with_b, south, a);

                    assert_eq!(b_above_a, a_below_b);
                }
            }
        }
    }

    /// Every tile in the set must be playable next to the opening position, or
    /// the discard-and-redraw rule would be doing work the bag design should
    /// have avoided. This is a property of the *distribution*, not of the code.
    #[test]
    fn every_tile_kind_is_placeable_next_to_the_opening_position() {
        let board = opening_board();
        for kind in TileKind::all() {
            assert!(
                has_any_legal_placement(&board, kind),
                "{} cannot be played next to the start tile",
                kind.def().name
            );
        }
    }

    #[test]
    fn enumeration_agrees_with_the_predicate() {
        let mut board = opening_board();
        board.insert(
            Coord::new(1, 0).unwrap(),
            PlacedTile::new(TileKind::new(3).unwrap(), Rotation::R90),
        );

        for kind in TileKind::all() {
            let enumerated = legal_placements(&board, kind);
            for coord in frontier(&board) {
                for rotation in Rotation::ALL {
                    let legal = is_legal_placement(&board, coord, PlacedTile::new(kind, rotation));
                    let listed = enumerated
                        .iter()
                        .any(|(at, rots)| *at == coord && rots.contains(&rotation));
                    assert_eq!(
                        legal,
                        listed,
                        "{} at {coord:?} {rotation:?}",
                        kind.def().name
                    );
                }
            }
            assert!(
                enumerated.iter().all(|(_, rots)| !rots.is_empty()),
                "an entry with no legal rotation is not a highlightable target"
            );
            assert_eq!(
                has_any_legal_placement(&board, kind),
                !enumerated.is_empty()
            );
            assert_eq!(
                first_legal_placement(&board, kind),
                enumerated.first().map(|(coord, rots)| (*coord, rots[0]))
            );
        }
    }

    #[test]
    fn the_frontier_is_exactly_the_free_squares_touching_the_board() {
        let board = opening_board();
        let frontier = frontier(&board);
        assert_eq!(frontier.len(), 4);
        for coord in &frontier {
            assert!(!board.contains(*coord));
            assert!(coord.orthogonal().any(|(_, n)| board.contains(n)));
        }
        // Canonically ordered and deduplicated.
        assert!(frontier.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn a_square_between_two_tiles_must_match_both_of_them() {
        // Start tile at origin, and a road-straight running north-south at
        // (0, -2). The square at (0, -1) must match the start tile's city to
        // its south and the road tile's road to its north.
        let mut board = opening_board();
        let road_straight = TileKind::new(3).unwrap();
        assert_eq!(road_straight.def().name, "road-straight");
        board.insert(
            Coord::new(0, -2).unwrap(),
            PlacedTile::new(road_straight, Rotation::R0),
        );

        let gap = Coord::new(0, -1).unwrap();
        for kind in TileKind::all() {
            for rotation in Rotation::ALL {
                let tile = PlacedTile::new(kind, rotation);
                let expected = tile.terrain(Side::South) == Terrain::City
                    && tile.terrain(Side::North) == Terrain::Road;
                assert_eq!(is_legal_placement(&board, gap, tile), expected);
            }
        }
    }
}
