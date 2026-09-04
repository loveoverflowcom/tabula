//! Tiles, their features, and the board they sit on.
//!
//! # Why a tile is a list of segments rather than four edge letters
//!
//! Four edge terrains would describe adjacency perfectly and the feature graph
//! not at all. A tile whose north and south edges are both city says nothing
//! about whether those are *one* city passing through or *two* separate ones —
//! and that distinction is the whole of Carcassonne-like scoring. So a tile is
//! a list of [`SegmentDef`]s, each naming a feature and the tile edges it
//! reaches; edge terrain is *derived* from them ([`TileDef::terrain`]) so the
//! two can never disagree.
//!
//! A road junction falls out of the same representation: three road segments of
//! one edge each, rather than one segment of three edges. Nothing special is
//! needed to say "these roads meet here but do not continue".
//!
//! # The tile distribution
//!
//! [`TILE_SET`] is Tabula's own, in the Carcassonne family. It is not a
//! reproduction of any published set. It is fixed, and changing it changes
//! `RULES_VERSION`, because a different bag is a different game.
//!
//! @ai.role domain-types
//! @ai.domain tiles.rules.tile
//! @ai.pure true
//! @ai.invariant tile-set-is-structurally-well-formed
//! @ai.invariant edge-terrain-is-derived-from-segments
//! @ai.evidence tests::every_tile_definition_is_structurally_well_formed
//! @ai.evidence tests::rotating_a_tile_permutes_its_edge_terrain

#![allow(clippy::doc_markdown)]

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::coord::{Coord, Rotation, Side};

/// What one tile edge shows. Two tiles may sit side by side only when the
/// terrains facing each other are equal.
///
/// `Field` is an edge terrain and **not** a [`FeatureKind`]: farms are out of
/// scope for Phase 3 (see the crate docs), so a field is matched but never
/// scored and never carries a follower.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Terrain {
    Field,
    Road,
    City,
}

/// A thing a follower can be placed on and that can be scored.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum FeatureKind {
    Road,
    City,
    Monastery,
}

impl FeatureKind {
    /// The edge terrain this feature shows where it reaches a tile edge.
    /// A monastery reaches no edge, so it shows none.
    #[must_use]
    pub const fn edge_terrain(self) -> Option<Terrain> {
        match self {
            Self::Road => Some(Terrain::Road),
            Self::City => Some(Terrain::City),
            Self::Monastery => None,
        }
    }

    // Point values deliberately live in `super::scoring` and nowhere else: a
    // `points_per_tile` constant here would be right for roads and cities and
    // wrong for a monastery, which is worth nine while covering one tile.
}

/// One feature as it appears on a single tile: a kind, the tile edges it
/// reaches, and whether it carries a pennant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SegmentDef {
    pub kind: FeatureKind,
    /// Tile-local sides this segment reaches, ascending and without repeats.
    /// Empty for a monastery.
    pub edges: &'static [Side],
    /// City pennants are worth an extra tile's score. Never set on a road.
    pub pennant: bool,
}

/// A tile kind's static definition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TileDef {
    /// Stable, human-readable id. Used in diagnostics and a11y text, never as
    /// a canonical value — [`TileKind`] is the canonical form.
    pub name: &'static str,
    pub segments: &'static [SegmentDef],
    /// How many of this kind the bag holds. The start tile is placed by
    /// `create` and is **not** counted here.
    pub count: u8,
}

impl TileDef {
    /// The terrain shown on `side` of an unrotated tile of this kind.
    #[must_use]
    pub fn terrain(&self, side: Side) -> Terrain {
        self.segments
            .iter()
            .find(|segment| segment.edges.contains(&side))
            .and_then(|segment| segment.kind.edge_terrain())
            .unwrap_or(Terrain::Field)
    }

    /// How many tile edges segment `index` reaches — the open-edge count a
    /// freshly placed segment starts with, before neighbours close any.
    #[must_use]
    pub fn open_edges(&self, index: usize) -> u32 {
        self.segments.get(index).map_or(0, |segment| {
            u32::try_from(segment.edges.len()).unwrap_or(u32::MAX)
        })
    }
}

const fn road(edges: &'static [Side]) -> SegmentDef {
    SegmentDef {
        kind: FeatureKind::Road,
        edges,
        pennant: false,
    }
}

const fn city(edges: &'static [Side], pennant: bool) -> SegmentDef {
    SegmentDef {
        kind: FeatureKind::City,
        edges,
        pennant,
    }
}

const MONASTERY: SegmentDef = SegmentDef {
    kind: FeatureKind::Monastery,
    edges: &[],
    pennant: false,
};

use Side::{East, North, South, West};

/// Every tile kind, in canonical order. The index into this table **is** the
/// canonical [`TileKind`] value, so rows may be appended but never reordered
/// or removed without a `RULES_VERSION` bump.
pub static TILE_SET: &[TileDef] = &[
    // 0 — also the start tile.
    TileDef {
        name: "city-cap-road",
        segments: &[city(&[North], false), road(&[East, West])],
        count: 4,
    },
    TileDef {
        name: "monastery",
        segments: &[MONASTERY],
        count: 4,
    },
    TileDef {
        name: "monastery-road",
        segments: &[MONASTERY, road(&[South])],
        count: 2,
    },
    TileDef {
        name: "road-straight",
        segments: &[road(&[North, South])],
        count: 8,
    },
    TileDef {
        name: "road-curve",
        segments: &[road(&[North, West])],
        count: 9,
    },
    TileDef {
        name: "road-junction",
        segments: &[road(&[North]), road(&[East]), road(&[West])],
        count: 4,
    },
    TileDef {
        name: "road-crossroad",
        segments: &[road(&[North]), road(&[East]), road(&[South]), road(&[West])],
        count: 1,
    },
    TileDef {
        name: "city-cap",
        segments: &[city(&[North], false)],
        count: 5,
    },
    TileDef {
        name: "city-cap-crossroad",
        segments: &[
            city(&[North], false),
            road(&[East]),
            road(&[South]),
            road(&[West]),
        ],
        count: 3,
    },
    TileDef {
        name: "city-through",
        segments: &[city(&[North, South], false)],
        count: 3,
    },
    TileDef {
        name: "city-corner",
        segments: &[city(&[North, West], false)],
        count: 3,
    },
    TileDef {
        name: "city-corner-pennant",
        segments: &[city(&[North, West], true)],
        count: 2,
    },
    TileDef {
        name: "city-corner-road",
        segments: &[city(&[North, West], false), road(&[East, South])],
        count: 3,
    },
    TileDef {
        name: "city-corner-road-pennant",
        segments: &[city(&[North, West], true), road(&[East, South])],
        count: 2,
    },
    TileDef {
        name: "city-three",
        segments: &[city(&[North, East, West], false)],
        count: 3,
    },
    TileDef {
        name: "city-three-pennant",
        segments: &[city(&[North, East, West], true)],
        count: 1,
    },
    TileDef {
        name: "city-three-road",
        segments: &[city(&[North, East, West], false), road(&[South])],
        count: 1,
    },
    TileDef {
        name: "city-three-road-pennant",
        segments: &[city(&[North, East, West], true), road(&[South])],
        count: 2,
    },
    TileDef {
        name: "city-full",
        segments: &[city(&[North, East, South, West], true)],
        count: 1,
    },
    TileDef {
        name: "city-two-opposite",
        segments: &[city(&[North], false), city(&[South], false)],
        count: 2,
    },
    TileDef {
        name: "city-two-adjacent",
        segments: &[city(&[North], false), city(&[East], false)],
        count: 2,
    },
    TileDef {
        name: "city-cap-road-right",
        segments: &[city(&[North], false), road(&[East, South])],
        count: 3,
    },
    TileDef {
        name: "city-cap-road-left",
        segments: &[city(&[North], false), road(&[South, West])],
        count: 3,
    },
];

/// The kind placed at [`Coord::ORIGIN`] by `create`, face up, unrotated.
pub const START_TILE: TileKind = TileKind(0);

/// How many tiles the bag holds at match start: the sum of every
/// [`TileDef::count`]. The start tile is placed separately, so the full set is
/// `BAG_SIZE + 1`.
pub const BAG_SIZE: usize = {
    let mut total = 0usize;
    let mut index = 0usize;
    while index < TILE_SET.len() {
        total += TILE_SET[index].count as usize;
        index += 1;
    }
    total
};

/// The largest number of segments any tile kind has. Bounds the segment index
/// space and therefore the size of a `SegmentRef`.
pub const MAX_SEGMENTS: u8 = 4;

/// An index into [`TILE_SET`], validated on construction **and on decode**.
///
/// This is the type that makes `TILE_SET[kind]` a total lookup: a hostile wire
/// value can never index past the table, so the rules never need a bounds
/// check or an `unwrap` to read a tile's definition (contract R3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub struct TileKind(u8);

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("tile kind is not in the tile set")]
pub struct TileKindError;

impl TileKind {
    /// The number of distinct tile kinds. `TILE_SET` is a static table with
    /// far fewer than 256 rows, which every index conversion here relies on.
    /// A `TileKind` is a `u8`, so a table longer than 255 rows would make some
    /// kinds unnameable. The `assert!` runs at compile time (this is a `const`
    /// initializer), which is what makes the cast below unable to truncate.
    #[allow(clippy::cast_possible_truncation)]
    const COUNT: u8 = {
        assert!(
            TILE_SET.len() <= u8::MAX as usize,
            "TILE_SET has more rows than a TileKind can name"
        );
        TILE_SET.len() as u8
    };

    /// # Errors
    /// [`TileKindError`] when `index` names no row of [`TILE_SET`].
    pub fn new(index: u8) -> Result<Self, TileKindError> {
        if index < Self::COUNT {
            Ok(Self(index))
        } else {
            Err(TileKindError)
        }
    }

    #[must_use]
    pub const fn index(self) -> u8 {
        self.0
    }

    /// Total by construction: [`TileKind`] can only name a row that exists.
    #[must_use]
    pub fn def(self) -> &'static TileDef {
        &TILE_SET[usize::from(self.0)]
    }

    /// Every kind, in canonical order.
    pub fn all() -> impl Iterator<Item = Self> {
        (0..Self::COUNT).map(Self)
    }
}

impl TryFrom<u8> for TileKind {
    type Error = TileKindError;

    fn try_from(index: u8) -> Result<Self, Self::Error> {
        Self::new(index)
    }
}

impl From<TileKind> for u8 {
    fn from(kind: TileKind) -> Self {
        kind.0
    }
}

/// A tile on the board: which kind, turned which way.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PlacedTile {
    pub kind: TileKind,
    pub rotation: Rotation,
}

impl PlacedTile {
    #[must_use]
    pub const fn new(kind: TileKind, rotation: Rotation) -> Self {
        Self { kind, rotation }
    }

    /// The terrain this tile shows on a **world-space** `side`.
    #[must_use]
    pub fn terrain(self, side: Side) -> Terrain {
        self.kind.def().terrain(side.unrotated(self.rotation))
    }

    #[must_use]
    pub fn segment_count(self) -> u8 {
        // `MAX_SEGMENTS` bounds this, and the tile-set test enforces it.
        u8::try_from(self.kind.def().segments.len()).unwrap_or(MAX_SEGMENTS)
    }

    /// The world-space sides segment `index` reaches, ascending.
    ///
    /// Empty for a monastery, and empty for an index this tile does not have —
    /// callers that must distinguish those use [`PlacedTile::segment`].
    pub fn segment_edges(self, index: u8) -> impl Iterator<Item = Side> {
        let rotation = self.rotation;
        self.segment(index)
            .map_or(&[][..], |segment| segment.edges)
            .iter()
            .map(move |side| side.rotated(rotation))
    }

    /// Segment `index` of this tile, or `None` if it has no such segment.
    #[must_use]
    pub fn segment(self, index: u8) -> Option<&'static SegmentDef> {
        self.kind.def().segments.get(usize::from(index))
    }

    /// The tile-local segment that reaches a **world-space** `side`, if any.
    #[must_use]
    pub fn segment_at(self, side: Side) -> Option<u8> {
        let local = side.unrotated(self.rotation);
        self.kind
            .def()
            .segments
            .iter()
            .position(|segment| segment.edges.contains(&local))
            .and_then(|index| u8::try_from(index).ok())
    }
}

/// The placed tiles, keyed by square.
///
/// Read access is public because the board is entirely public information — it
/// is the same value in `State` and in `View`, which is what lets one legality
/// function serve the rules, the bots, and the presenter. Mutation is
/// crate-private, so nothing outside `rules` can put a tile down without going
/// through [`super::placement`].
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Board(BTreeMap<Coord, PlacedTile>);

impl Board {
    #[must_use]
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }

    #[must_use]
    pub fn get(&self, coord: Coord) -> Option<PlacedTile> {
        self.0.get(&coord).copied()
    }

    #[must_use]
    pub fn contains(&self, coord: Coord) -> bool {
        self.0.contains_key(&coord)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Every placed tile in canonical `(x, y)` order.
    pub fn iter(&self) -> impl Iterator<Item = (Coord, PlacedTile)> + '_ {
        self.0.iter().map(|(coord, tile)| (*coord, *tile))
    }

    /// How many of `coord`'s eight surrounding squares are occupied.
    #[must_use]
    pub fn surrounding_count(&self, coord: Coord) -> u32 {
        // At most eight; the conversion cannot fail.
        u32::try_from(
            coord
                .surrounding()
                .filter(|neighbour| self.contains(*neighbour))
                .count(),
        )
        .unwrap_or(8)
    }

    pub(super) fn insert(&mut self, coord: Coord, tile: PlacedTile) {
        self.0.insert(coord, tile);
    }
}

impl<'a> IntoIterator for &'a Board {
    type Item = (Coord, PlacedTile);
    type IntoIter = std::iter::Map<
        std::collections::btree_map::Iter<'a, Coord, PlacedTile>,
        fn((&'a Coord, &'a PlacedTile)) -> (Coord, PlacedTile),
    >;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter().map(|(coord, tile)| (*coord, *tile))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    /// The static table is data, and data can be wrong in ways the type system
    /// does not see. This walks every row and asserts the four structural facts
    /// the rest of the crate assumes without re-checking.
    #[test]
    fn every_tile_definition_is_structurally_well_formed() {
        for kind in TileKind::all() {
            let def = kind.def();
            let label = def.name;
            assert!(
                !def.segments.is_empty(),
                "{label}: a tile with no segments can never be scored or claimed"
            );
            assert!(
                def.segments.len() <= usize::from(MAX_SEGMENTS),
                "{label}: has more segments than MAX_SEGMENTS allows"
            );
            assert!(
                def.count >= 1,
                "{label}: a kind with no copies is dead data"
            );

            let mut claimed: BTreeSet<Side> = BTreeSet::new();
            for segment in def.segments {
                let mut previous: Option<Side> = None;
                for side in segment.edges {
                    assert!(
                        previous.is_none_or(|last| last < *side),
                        "{label}: segment edges must be ascending and unique"
                    );
                    previous = Some(*side);
                    assert!(
                        claimed.insert(*side),
                        "{label}: side {side:?} belongs to two segments, so its edge \
                         terrain would be ambiguous"
                    );
                }
                match segment.kind {
                    FeatureKind::Monastery => assert!(
                        segment.edges.is_empty(),
                        "{label}: a monastery reaches no edge"
                    ),
                    FeatureKind::Road | FeatureKind::City => assert!(
                        !segment.edges.is_empty(),
                        "{label}: a road or city that reaches no edge can never connect"
                    ),
                }
                assert!(
                    !segment.pennant || segment.kind == FeatureKind::City,
                    "{label}: only a city carries a pennant"
                );
            }
        }
    }

    #[test]
    fn the_bag_and_the_full_set_are_the_documented_sizes() {
        assert_eq!(BAG_SIZE, 71);
        assert_eq!(BAG_SIZE + 1, 72, "the start tile completes the 72-tile set");
    }

    /// Edge terrain is derived, so it cannot disagree with the segments — this
    /// checks the derivation itself, over the whole table.
    #[test]
    fn edge_terrain_agrees_with_the_segment_that_reaches_that_edge() {
        for kind in TileKind::all() {
            let def = kind.def();
            for side in Side::ALL {
                let owning = def
                    .segments
                    .iter()
                    .find(|segment| segment.edges.contains(&side));
                let expected = match owning {
                    Some(segment) => segment.kind.edge_terrain().expect("edge-reaching kind"),
                    None => Terrain::Field,
                };
                assert_eq!(def.terrain(side), expected, "{}", def.name);
            }
        }
    }

    /// The rotation law, over the whole tile set and all four rotations: a
    /// rotated tile shows on `s.rotated(r)` exactly what the unrotated tile
    /// shows on `s`.
    #[test]
    fn rotating_a_tile_permutes_its_edge_terrain() {
        for kind in TileKind::all() {
            for rotation in Rotation::ALL {
                let placed = PlacedTile::new(kind, rotation);
                for side in Side::ALL {
                    assert_eq!(
                        placed.terrain(side.rotated(rotation)),
                        kind.def().terrain(side),
                        "{} at {rotation:?}",
                        kind.def().name
                    );
                }
            }
        }
    }

    #[test]
    fn segment_lookup_by_world_side_is_the_inverse_of_segment_edges() {
        for kind in TileKind::all() {
            for rotation in Rotation::ALL {
                let placed = PlacedTile::new(kind, rotation);
                for index in 0..placed.segment_count() {
                    for side in placed.segment_edges(index) {
                        assert_eq!(placed.segment_at(side), Some(index));
                    }
                }
                for side in Side::ALL {
                    match placed.segment_at(side) {
                        Some(index) => {
                            assert!(placed.segment_edges(index).any(|edge| edge == side));
                        }
                        None => assert_eq!(placed.terrain(side), Terrain::Field),
                    }
                }
            }
        }
    }

    #[test]
    fn tile_kind_rejects_indices_outside_the_table_including_from_the_wire() {
        assert!(TileKind::new(0).is_ok());
        let last = u8::try_from(TILE_SET.len() - 1).expect("the table fits in a TileKind");
        assert!(TileKind::new(last).is_ok());
        assert_eq!(TileKind::new(last + 1), Err(TileKindError));
        assert_eq!(TileKind::new(u8::MAX), Err(TileKindError));

        let hostile = tabula_core::canonical_encode(&(last + 1)).unwrap();
        assert!(tabula_core::canonical_decode::<TileKind>(&hostile).is_err());
    }

    #[test]
    fn the_start_tile_shows_one_city_edge_and_a_road_through() {
        let start = PlacedTile::new(START_TILE, Rotation::R0);
        assert_eq!(start.terrain(Side::North), Terrain::City);
        assert_eq!(start.terrain(Side::East), Terrain::Road);
        assert_eq!(start.terrain(Side::South), Terrain::Field);
        assert_eq!(start.terrain(Side::West), Terrain::Road);
    }

    /// Every edge terrain must be reachable from the start tile, or a tile
    /// showing it could never be played next to the opening position.
    #[test]
    fn the_start_tile_offers_every_edge_terrain_to_its_neighbours() {
        let start = PlacedTile::new(START_TILE, Rotation::R0);
        let offered: BTreeSet<Terrain> = Side::ALL.into_iter().map(|s| start.terrain(s)).collect();
        assert_eq!(
            offered,
            BTreeSet::from([Terrain::Field, Terrain::Road, Terrain::City])
        );
    }
}
