//! The coordinate system: [`Coord`], [`Side`], [`Rotation`].
//!
//! Three small types with one job between them: make "which square is north of
//! this one, on a tile turned a quarter turn clockwise" a question the compiler
//! can check rather than one every call site re-derives from `i16` arithmetic.
//!
//! @ai.role domain-types
//! @ai.domain tiles.rules.coord
//! @ai.pure true
//! @ai.invariant coord-inside-playable-space
//! @ai.law rotation-is-a-cyclic-group-of-order-four
//! @ai.evidence tests::rotating_a_side_four_times_is_the_identity
//! @ai.evidence tests::neighbour_and_opposite_side_are_mutually_inverse

#![allow(clippy::doc_markdown)]

use serde::{Deserialize, Serialize};

/// Half-width of the playable coordinate space.
///
/// A legal board can never leave `±(BAG_SIZE - 1)` of the origin, because every
/// placed tile must touch one already on the board and the bag holds
/// [`crate::rules::tile::BAG_SIZE`] tiles. The bound here is comfortably larger
/// than that and comfortably smaller than `i16::MAX`, which is the point: it is
/// a *decode-time* filter, not a rule. A `Command::PlaceTile` arriving from the
/// wire with `x = 32_767` is refused by [`Coord`]'s own deserializer, so no
/// later neighbour computation can overflow on it.
pub const MAX_COORD: i16 = 128;

/// A square on the unbounded board, validated to lie inside the playable
/// coordinate space.
///
/// `y` increases **downward** (screen convention), so [`Side::North`] is
/// `y - 1`. That choice is arbitrary but it is made exactly once, here, and
/// [`Coord::neighbour`] is the only place it is spelled out.
///
/// `Ord` is derived and is `(x, y)` lexicographic. Every ordered iteration over
/// the board — event order, hint order, feature membership — inherits it, which
/// is what makes those orders canonical (I-2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "RawCoord", into = "RawCoord")]
pub struct Coord {
    x: i16,
    y: i16,
}

/// The unvalidated wire/storage shape of a [`Coord`].
#[derive(Clone, Copy, Serialize, Deserialize)]
struct RawCoord {
    x: i16,
    y: i16,
}

/// Why a coordinate is not usable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("coordinate is outside the playable space (|x|, |y| must be <= {MAX_COORD})")]
pub struct CoordError;

impl Coord {
    /// The square the first tile occupies.
    pub const ORIGIN: Self = Self { x: 0, y: 0 };

    /// # Errors
    /// [`CoordError`] when either axis leaves the playable space.
    ///
    /// Written as a range check rather than `x.abs() > MAX_COORD`: `abs()`
    /// panics on `i16::MIN`, and `i16::MIN` is exactly the sort of value a
    /// hostile `PlaceTile` would carry (contract R3). The range form is total.
    pub fn new(x: i16, y: i16) -> Result<Self, CoordError> {
        const RANGE: core::ops::RangeInclusive<i16> = -MAX_COORD..=MAX_COORD;
        if !RANGE.contains(&x) || !RANGE.contains(&y) {
            return Err(CoordError);
        }
        Ok(Self { x, y })
    }

    #[must_use]
    pub const fn x(self) -> i16 {
        self.x
    }

    #[must_use]
    pub const fn y(self) -> i16 {
        self.y
    }

    /// The orthogonally adjacent square on `side`.
    ///
    /// `None` only at the very edge of the playable space. A legal match never
    /// reaches it — the bag is far too small — but returning `Option` is what
    /// keeps this function total for hostile input rather than relying on that
    /// argument holding forever (contract R3).
    #[must_use]
    pub fn neighbour(self, side: Side) -> Option<Self> {
        let (dx, dy) = match side {
            Side::North => (0, -1),
            Side::East => (1, 0),
            Side::South => (0, 1),
            Side::West => (-1, 0),
        };
        Self::new(self.x.checked_add(dx)?, self.y.checked_add(dy)?).ok()
    }

    /// The four orthogonal neighbours, in [`Side`] order.
    pub fn orthogonal(self) -> impl Iterator<Item = (Side, Self)> {
        Side::ALL
            .into_iter()
            .filter_map(move |side| self.neighbour(side).map(|coord| (side, coord)))
    }

    /// The eight surrounding squares, ordered. A monastery is complete when all
    /// eight are occupied, which is the only rule that needs diagonals.
    pub fn surrounding(self) -> impl Iterator<Item = Self> {
        const OFFSETS: [(i16, i16); 8] = [
            (-1, -1),
            (0, -1),
            (1, -1),
            (-1, 0),
            (1, 0),
            (-1, 1),
            (0, 1),
            (1, 1),
        ];
        OFFSETS.into_iter().filter_map(move |(dx, dy)| {
            Self::new(self.x.checked_add(dx)?, self.y.checked_add(dy)?).ok()
        })
    }
}

impl TryFrom<RawCoord> for Coord {
    type Error = CoordError;

    fn try_from(raw: RawCoord) -> Result<Self, Self::Error> {
        Self::new(raw.x, raw.y)
    }
}

impl From<Coord> for RawCoord {
    fn from(coord: Coord) -> Self {
        Self {
            x: coord.x,
            y: coord.y,
        }
    }
}

/// One of a tile's four edges, clockwise from the top.
///
/// The discriminants are load-bearing: [`Rotation`] and [`Side::opposite`] are
/// both modular arithmetic on them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum Side {
    North = 0,
    East = 1,
    South = 2,
    West = 3,
}

impl Side {
    /// Every side, in clockwise order. Iterating this is canonical (I-2).
    pub const ALL: [Self; 4] = [Self::North, Self::East, Self::South, Self::West];

    #[must_use]
    pub const fn index(self) -> u8 {
        self as u8
    }

    /// The only constructor from a raw index. Total: it takes the residue, so
    /// every `u8` maps to a side and no caller needs an `unreachable!`.
    #[must_use]
    pub const fn from_index(index: u8) -> Self {
        match index % 4 {
            0 => Self::North,
            1 => Self::East,
            2 => Self::South,
            _ => Self::West,
        }
    }

    /// The side that faces this one across a shared edge. Two tiles are
    /// adjacency-compatible when `a.terrain(side) == b.terrain(side.opposite())`.
    #[must_use]
    pub const fn opposite(self) -> Self {
        Self::from_index(self.index() + 2)
    }

    /// Where this side ends up after the tile is turned clockwise by `rotation`.
    #[must_use]
    pub const fn rotated(self, rotation: Rotation) -> Self {
        Self::from_index(self.index() + rotation.quarter_turns())
    }

    /// The side that *was* here before the tile was turned by `rotation` — the
    /// inverse of [`Side::rotated`], and how a world-space side is mapped back
    /// to a tile-local one.
    #[must_use]
    pub const fn unrotated(self, rotation: Rotation) -> Self {
        Self::from_index(self.index() + 4 - rotation.quarter_turns())
    }
}

/// A tile's orientation: quarter turns clockwise.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[repr(u8)]
pub enum Rotation {
    #[default]
    R0 = 0,
    R90 = 1,
    R180 = 2,
    R270 = 3,
}

impl Rotation {
    /// Every rotation, in increasing order. Iterating this is canonical (I-2).
    pub const ALL: [Self; 4] = [Self::R0, Self::R90, Self::R180, Self::R270];

    #[must_use]
    pub const fn quarter_turns(self) -> u8 {
        self as u8
    }

    /// Total for every `u8`, for the same reason as [`Side::from_index`].
    #[must_use]
    pub const fn from_quarter_turns(turns: u8) -> Self {
        match turns % 4 {
            0 => Self::R0,
            1 => Self::R90,
            2 => Self::R180,
            _ => Self::R270,
        }
    }

    /// One more quarter turn clockwise — what the rotate control does.
    #[must_use]
    pub const fn next(self) -> Self {
        Self::from_quarter_turns(self.quarter_turns() + 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coord_rejects_positions_outside_the_playable_space() {
        assert_eq!(
            Coord::new(MAX_COORD, MAX_COORD).map(Coord::x),
            Ok(MAX_COORD)
        );
        assert_eq!(Coord::new(MAX_COORD + 1, 0), Err(CoordError));
        assert_eq!(Coord::new(0, -MAX_COORD - 1), Err(CoordError));
        // `i16::MIN` is the value that makes the obvious `abs()` spelling
        // panic instead of reject (contract R3).
        assert_eq!(Coord::new(i16::MIN, 0), Err(CoordError));
        assert_eq!(Coord::new(0, i16::MIN), Err(CoordError));
        assert_eq!(Coord::new(i16::MAX, i16::MIN), Err(CoordError));
    }

    /// Deserialization must not be a second, unvalidated constructor
    /// (`rust-types-as-proofs`: serde bypasses smart constructors by default).
    #[test]
    fn coord_deserialization_cannot_bypass_the_playable_space_check() {
        let hostile = tabula_core::canonical_encode(&RawCoord { x: 30_000, y: 0 })
            .expect("the raw shape encodes");
        assert!(tabula_core::canonical_decode::<Coord>(&hostile).is_err());

        let legal = tabula_core::canonical_encode(&Coord::new(3, -4).unwrap()).unwrap();
        assert_eq!(
            tabula_core::canonical_decode::<Coord>(&legal).unwrap(),
            Coord::new(3, -4).unwrap()
        );
    }

    #[test]
    fn rotating_a_side_four_times_is_the_identity() {
        for side in Side::ALL {
            for rotation in Rotation::ALL {
                assert_eq!(side.rotated(rotation).unrotated(rotation), side);
            }
            let once = side.rotated(Rotation::R90);
            let twice = once.rotated(Rotation::R90);
            let thrice = twice.rotated(Rotation::R90);
            assert_eq!(thrice.rotated(Rotation::R90), side);
            assert_eq!(twice, side.rotated(Rotation::R180));
            assert_eq!(thrice, side.rotated(Rotation::R270));
        }
    }

    #[test]
    fn opposite_is_an_involution_and_never_the_identity() {
        for side in Side::ALL {
            assert_eq!(side.opposite().opposite(), side);
            assert_ne!(side.opposite(), side);
        }
    }

    /// The single fact the whole adjacency rule rests on: stepping to a
    /// neighbour and looking back along the opposite side returns you home.
    #[test]
    fn neighbour_and_opposite_side_are_mutually_inverse() {
        for x in -3..=3 {
            for y in -3..=3 {
                let coord = Coord::new(x, y).unwrap();
                for side in Side::ALL {
                    let neighbour = coord.neighbour(side).unwrap();
                    assert_eq!(neighbour.neighbour(side.opposite()), Some(coord));
                }
            }
        }
    }

    #[test]
    fn neighbour_is_total_at_the_boundary_instead_of_overflowing() {
        let corner = Coord::new(MAX_COORD, MAX_COORD).unwrap();
        assert_eq!(corner.neighbour(Side::East), None);
        assert_eq!(corner.neighbour(Side::South), None);
        assert!(corner.neighbour(Side::West).is_some());
        assert_eq!(corner.surrounding().count(), 3);
        assert_eq!(Coord::ORIGIN.surrounding().count(), 8);
    }

    #[test]
    fn rotation_next_cycles_through_all_four_orientations() {
        let mut seen = Vec::new();
        let mut rotation = Rotation::R0;
        for _ in 0..4 {
            seen.push(rotation);
            rotation = rotation.next();
        }
        assert_eq!(rotation, Rotation::R0);
        assert_eq!(seen, Rotation::ALL);
    }

    #[test]
    fn raw_index_constructors_are_total_over_every_byte() {
        for byte in 0..=u8::MAX {
            assert_eq!(Side::from_index(byte).index(), byte % 4);
            assert_eq!(Rotation::from_quarter_turns(byte).quarter_turns(), byte % 4);
        }
    }
}
