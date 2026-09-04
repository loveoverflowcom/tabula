//! The feature graph: which placed segments form one road, city, or monastery,
//! how many edges each still has open, and who has a follower on it.
//!
//! # Choosing the representation
//!
//! Doc 02 §12.4 sketched "incremental union-find". Four representations were
//! compared against the five properties the sketch itself demands —
//! deterministic, canonical-serialization safe, incrementally maintainable,
//! easy to validate, hard to corrupt — because the structure has to live *in*
//! `State` and therefore *in* the state hash, where a representation that is
//! not a function of the semantic state stops being a divergence detector and
//! starts being a divergence source.
//!
//! | Representation | Why not / why |
//! |---|---|
//! | **Union-find with path compression** | `find` **mutates**. `project` and `legal_commands` are read paths; running them would change the encoded bytes, so two runs of the same input stream could hash differently purely from how often something was queried. Disabling compression removes the mutation and most of the benefit, and leaves parent pointers that still encode union order rather than membership. |
//! | **Recompute the whole board per query** | Provably canonical and obviously correct, and it is the *reference model* this module is tested against (`tests/features.rs`). As production code it re-walks the board on every placement, which is precisely the cost doc 02 §3.3 says `&mut State` exists to avoid. |
//! | **Segment→segment adjacency list, components derived on demand** | Canonical, but every closure and ownership question becomes a traversal, and follower ownership has to be aggregated per query — more derivation, and more places for two answers to disagree. |
//! | **Explicit component registry, merged by minimum id** — chosen | Reads never mutate. A component's contents are a set union and its id is a minimum, so both are order-independent: processing the four sides of a new tile in any order lands on the same bytes. Closure is a single counter reaching zero. Ownership and pennants are stored where they are read. |
//!
//! ## What "canonical" does and does not claim here
//!
//! The encoding is a function of the **input sequence**, which is what
//! determinism and replay require (I-2): the same seed and the same ordered
//! inputs always produce the same bytes. It is deliberately *not* a function of
//! the board alone — feature ids are handed out in placement order, so two
//! different orders reaching the same board hold the same components under
//! different ids. Nothing in the contract asks for more than the former, and
//! asking for the latter would mean re-canonicalising ids on every placement
//! for no reader's benefit.
//!
//! ## Cost
//!
//! A merge moves the smaller history into the surviving component, so it is
//! `O(k log k)` in that component's size, and only components *adjacent to the
//! new tile* are ever touched — never the whole board. With 72 tiles of at most
//! four segments each the entire match is a few thousand `BTreeMap` operations.
//! Measured rather than assumed: `tests/state_size.rs` records the encoding, and
//! wall-clock budgets belong to the Phase-4 load test (doc 06 §2).
//!
//! @ai.role domain-types
//! @ai.domain tiles.rules.feature
//! @ai.pure true
//! @ai.invariant every-placed-segment-belongs-to-exactly-one-feature
//! @ai.invariant open-edge-count-equals-a-whole-board-recount
//! @ai.invariant a-scored-feature-holds-no-followers
//! @ai.law incremental-graph-agrees-with-whole-board-recomputation
//! @ai.evidence tests::merging_is_independent_of_the_order_sides_are_processed
//! @ai.evidence crate::rules::feature::tests::a_completed_feature_reports_no_open_edges

#![allow(clippy::doc_markdown)]

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use tabula_core::SeatId;

use super::coord::Coord;
use super::tile::{Board, FeatureKind, MAX_SEGMENTS};

/// A monastery is complete when every one of its eight surrounding squares
/// holds a tile.
pub const MONASTERY_NEIGHBOURS: u32 = 8;

/// One feature as it sits on one tile: the square, and which of that tile's
/// segments.
///
/// Ordered by `(coord, index)`, so every iteration over a component's members —
/// event order, follower rendering, scoring — is canonical (I-2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "RawSegmentRef", into = "RawSegmentRef")]
pub struct SegmentRef {
    coord: Coord,
    index: u8,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
struct RawSegmentRef {
    coord: Coord,
    index: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("segment index is out of range for any tile (max {MAX_SEGMENTS})")]
pub struct SegmentRefError;

impl SegmentRef {
    /// # Errors
    /// [`SegmentRefError`] when `index` exceeds what any tile kind can have.
    ///
    /// Whether the tile *at this square* really has that segment is a
    /// cross-field question, checked by [`FeatureGraph::check_against`]. This
    /// bound is the standalone one, and it is what keeps a hostile wire value
    /// out of an index computation.
    pub fn new(coord: Coord, index: u8) -> Result<Self, SegmentRefError> {
        if index < MAX_SEGMENTS {
            Ok(Self { coord, index })
        } else {
            Err(SegmentRefError)
        }
    }

    #[must_use]
    pub const fn coord(self) -> Coord {
        self.coord
    }

    #[must_use]
    pub const fn index(self) -> u8 {
        self.index
    }
}

impl TryFrom<RawSegmentRef> for SegmentRef {
    type Error = SegmentRefError;

    fn try_from(raw: RawSegmentRef) -> Result<Self, Self::Error> {
        Self::new(raw.coord, raw.index)
    }
}

impl From<SegmentRef> for RawSegmentRef {
    fn from(value: SegmentRef) -> Self {
        Self {
            coord: value.coord,
            index: value.index,
        }
    }
}

/// A component's identity. Handed out by [`FeatureGraph`] in placement order
/// and never reused; construction is crate-private so an id can only name a
/// component the graph really created.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct FeatureId(u32);

impl FeatureId {
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// One connected road, city, or monastery.
///
/// Fields are private: the graph maintains cross-field invariants between
/// `members`, `open_edges`, and `meeples` that a caller reaching in could
/// break silently.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Feature {
    kind: FeatureKind,
    members: BTreeSet<SegmentRef>,
    /// Edge slots still facing an empty square. Zero means complete. For a
    /// monastery this counts unoccupied surrounding squares instead, so that
    /// "complete" is one rule and not two.
    open_edges: u32,
    pennants: u32,
    meeples: BTreeMap<SegmentRef, SeatId>,
    scored: bool,
}

impl Feature {
    #[must_use]
    pub const fn kind(&self) -> FeatureKind {
        self.kind
    }

    #[must_use]
    pub const fn members(&self) -> &BTreeSet<SegmentRef> {
        &self.members
    }

    #[must_use]
    pub const fn open_edges(&self) -> u32 {
        self.open_edges
    }

    #[must_use]
    pub const fn pennants(&self) -> u32 {
        self.pennants
    }

    #[must_use]
    pub const fn meeples(&self) -> &BTreeMap<SegmentRef, SeatId> {
        &self.meeples
    }

    #[must_use]
    pub const fn scored(&self) -> bool {
        self.scored
    }

    /// Complete: no edge of it faces an empty square.
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.open_edges == 0
    }

    /// Distinct squares this feature covers — the unit both scoring rules count.
    #[must_use]
    pub fn tiles(&self) -> BTreeSet<Coord> {
        self.members.iter().map(|member| member.coord).collect()
    }

    /// How many followers each seat has on it, ascending by seat.
    #[must_use]
    pub fn follower_counts(&self) -> BTreeMap<SeatId, u32> {
        let mut counts: BTreeMap<SeatId, u32> = BTreeMap::new();
        for seat in self.meeples.values() {
            *counts.entry(*seat).or_default() += 1;
        }
        counts
    }
}

/// Why a graph does not describe the board it is paired with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum FeatureGraphError {
    #[error("a placed segment belongs to no feature")]
    UnownedSegment,
    #[error("a feature claims a segment that is not on the board")]
    PhantomSegment,
    #[error("a feature's membership disagrees with the segment-to-feature index")]
    MembershipMismatch,
    #[error("a feature's kind disagrees with the segment it claims")]
    KindMismatch,
    #[error("a feature's open-edge count disagrees with the board")]
    OpenEdgeMismatch,
    #[error("a feature's pennant count disagrees with the board")]
    PennantMismatch,
    #[error("a follower sits on a segment its feature does not contain")]
    StrayFollower,
    #[error("a scored feature still holds followers")]
    ScoredFeatureHoldsFollowers,
    #[error("two segments that are connected on the board are in different features")]
    ConnectedSegmentsSplit,
    #[error("two segments that are not connected on the board share a feature")]
    UnconnectedSegmentsMerged,
}

/// The incremental component registry.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeatureGraph {
    next_id: u32,
    owner: BTreeMap<SegmentRef, FeatureId>,
    features: BTreeMap<FeatureId, Feature>,
}

impl FeatureGraph {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The component `segment` belongs to, if it is on the board.
    #[must_use]
    pub fn of(&self, segment: SegmentRef) -> Option<FeatureId> {
        self.owner.get(&segment).copied()
    }

    #[must_use]
    pub fn feature(&self, id: FeatureId) -> Option<&Feature> {
        self.features.get(&id)
    }

    /// Every component, ascending by id.
    pub fn iter(&self) -> impl Iterator<Item = (FeatureId, &Feature)> {
        self.features.iter().map(|(id, feature)| (*id, feature))
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.features.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.features.is_empty()
    }

    /// Where every follower on the board is sitting. Public information: it is
    /// what the presenter draws and what a bot reads out of its `View`.
    #[must_use]
    pub fn followers(&self) -> BTreeMap<SegmentRef, SeatId> {
        self.features
            .values()
            .flat_map(|feature| feature.meeples.iter().map(|(at, seat)| (*at, *seat)))
            .collect()
    }

    /// Whether a follower may be placed on `segment`: its component exists, is
    /// unscored, and nobody has claimed it yet.
    ///
    /// "Nobody has claimed it" is the whole of the classic rule — a feature
    /// gains a second owner only by *merging* with an already-claimed one, never
    /// by being claimed twice.
    #[must_use]
    pub fn is_claimable(&self, segment: SegmentRef) -> bool {
        self.of(segment)
            .and_then(|id| self.features.get(&id))
            .is_some_and(|feature| !feature.scored && feature.meeples.is_empty())
    }

    // -- mutation, all crate-private -------------------------------------

    fn fresh_id(&mut self) -> FeatureId {
        let id = FeatureId(self.next_id);
        self.next_id = self.next_id.saturating_add(1);
        id
    }

    /// Fold the tile at `coord` into the graph. `board` must already contain it.
    ///
    /// Returns every component the placement touched, ascending — the exact set
    /// that can have reached completion, which is what makes closure checking
    /// incremental rather than a board sweep.
    pub(super) fn place_tile(&mut self, board: &Board, coord: Coord) -> BTreeSet<FeatureId> {
        let Some(tile) = board.get(coord) else {
            return BTreeSet::new();
        };
        let mut touched = BTreeSet::new();

        for index in 0..tile.segment_count() {
            let Some(def) = tile.segment(index) else {
                continue;
            };
            let Ok(segment) = SegmentRef::new(coord, index) else {
                continue;
            };

            let mut open = 0u32;
            let mut neighbours: Vec<FeatureId> = Vec::new();
            for side in tile.segment_edges(index) {
                let Some(neighbour_coord) = coord.neighbour(side) else {
                    open += 1;
                    continue;
                };
                let Some(neighbour) = board.get(neighbour_coord) else {
                    open += 1;
                    continue;
                };
                // The placement rule already proved the terrains match, so the
                // neighbour has a segment facing us.
                let Some(neighbour_index) = neighbour.segment_at(side.opposite()) else {
                    open += 1;
                    continue;
                };
                let Ok(neighbour_segment) = SegmentRef::new(neighbour_coord, neighbour_index)
                else {
                    continue;
                };
                if let Some(id) = self.of(neighbour_segment) {
                    // That component's slot facing us just closed.
                    if let Some(feature) = self.features.get_mut(&id) {
                        feature.open_edges = feature.open_edges.saturating_sub(1);
                    }
                    neighbours.push(id);
                }
            }

            if def.kind == FeatureKind::Monastery {
                // A monastery has no edges; it closes when its eight
                // surrounding squares are filled. Counting the gaps as
                // "open edges" makes completion one rule, not two.
                open = MONASTERY_NEIGHBOURS.saturating_sub(board.surrounding_count(coord));
            }

            let id = self.fresh_id();
            self.features.insert(
                id,
                Feature {
                    kind: def.kind,
                    members: BTreeSet::from([segment]),
                    open_edges: open,
                    pennants: u32::from(def.pennant),
                    meeples: BTreeMap::new(),
                    scored: false,
                },
            );
            self.owner.insert(segment, id);

            let mut surviving = id;
            for neighbour_id in neighbours {
                surviving = self.merge(surviving, neighbour_id);
            }
            touched.insert(surviving);
        }

        // The new tile also fills one gap around every monastery near it.
        for neighbour_coord in coord.surrounding() {
            let Some(neighbour) = board.get(neighbour_coord) else {
                continue;
            };
            for index in 0..neighbour.segment_count() {
                if neighbour.segment(index).map(|def| def.kind) != Some(FeatureKind::Monastery) {
                    continue;
                }
                let Ok(segment) = SegmentRef::new(neighbour_coord, index) else {
                    continue;
                };
                if let Some(id) = self.of(segment) {
                    if let Some(feature) = self.features.get_mut(&id) {
                        feature.open_edges = feature.open_edges.saturating_sub(1);
                    }
                    touched.insert(id);
                }
            }
        }

        touched
    }

    /// Fuse two components, keeping the **lower** id. Returns the survivor.
    ///
    /// Minimum-id is what makes a placement's result independent of the order
    /// its sides happen to be processed in — see the module docs.
    fn merge(&mut self, left: FeatureId, right: FeatureId) -> FeatureId {
        if left == right {
            return left;
        }
        let (keep, drop) = if left < right {
            (left, right)
        } else {
            (right, left)
        };
        let Some(absorbed) = self.features.remove(&drop) else {
            return keep;
        };
        let Some(survivor) = self.features.get_mut(&keep) else {
            // Cannot happen: both ids came from `self`. Put it back rather
            // than losing a component if it somehow does.
            self.features.insert(drop, absorbed);
            return drop;
        };
        for member in &absorbed.members {
            self.owner.insert(*member, keep);
        }
        survivor.members.extend(absorbed.members);
        survivor.open_edges = survivor.open_edges.saturating_add(absorbed.open_edges);
        survivor.pennants = survivor.pennants.saturating_add(absorbed.pennants);
        survivor.meeples.extend(absorbed.meeples);
        survivor.scored |= absorbed.scored;
        keep
    }

    /// Put `seat`'s follower on `segment`. Returns false if that is not legal,
    /// having changed nothing.
    pub(super) fn place_follower(&mut self, segment: SegmentRef, seat: SeatId) -> bool {
        if !self.is_claimable(segment) {
            return false;
        }
        let Some(id) = self.of(segment) else {
            return false;
        };
        let Some(feature) = self.features.get_mut(&id) else {
            return false;
        };
        feature.meeples.insert(segment, seat);
        true
    }

    /// Mark a component scored and hand its followers back.
    ///
    /// Returns how many each seat gets back, ascending by seat. A second call
    /// on the same component returns nothing and changes nothing, which is what
    /// makes "a completed feature scores exactly once" hold even if a caller
    /// asks twice.
    pub(super) fn retire(&mut self, id: FeatureId) -> BTreeMap<SeatId, u32> {
        let Some(feature) = self.features.get_mut(&id) else {
            return BTreeMap::new();
        };
        if feature.scored {
            return BTreeMap::new();
        }
        feature.scored = true;
        let returned = feature.follower_counts();
        feature.meeples.clear();
        returned
    }

    // -- validation -------------------------------------------------------

    /// Check the graph against the board it claims to describe.
    ///
    /// This is the cross-field half of `State`'s validator: the graph alone
    /// cannot know whether a segment exists, and the board alone cannot know
    /// which components it was folded into. It re-derives connectivity, open
    /// edges, and pennants from the board and compares — which makes it an
    /// *independent recomputation*, the same oracle `tests/features.rs` uses,
    /// reached through the decode path instead of a test.
    ///
    /// # Errors
    /// The first disagreement found.
    pub fn check_against(&self, board: &Board) -> Result<(), FeatureGraphError> {
        // Every placed segment is owned exactly once, with a matching kind.
        let mut placed: BTreeSet<SegmentRef> = BTreeSet::new();
        for (coord, tile) in board.iter() {
            for index in 0..tile.segment_count() {
                let Ok(segment) = SegmentRef::new(coord, index) else {
                    continue;
                };
                placed.insert(segment);
                let Some(id) = self.of(segment) else {
                    return Err(FeatureGraphError::UnownedSegment);
                };
                let Some(feature) = self.features.get(&id) else {
                    return Err(FeatureGraphError::UnownedSegment);
                };
                if !feature.members.contains(&segment) {
                    return Err(FeatureGraphError::MembershipMismatch);
                }
                if tile.segment(index).map(|def| def.kind) != Some(feature.kind) {
                    return Err(FeatureGraphError::KindMismatch);
                }
            }
        }
        for (id, feature) in &self.features {
            for member in &feature.members {
                if !placed.contains(member) {
                    return Err(FeatureGraphError::PhantomSegment);
                }
                if self.owner.get(member) != Some(id) {
                    return Err(FeatureGraphError::MembershipMismatch);
                }
            }
            for follower_at in feature.meeples.keys() {
                if !feature.members.contains(follower_at) {
                    return Err(FeatureGraphError::StrayFollower);
                }
            }
            if feature.scored && !feature.meeples.is_empty() {
                return Err(FeatureGraphError::ScoredFeatureHoldsFollowers);
            }
        }
        if self.owner.len() != placed.len() {
            return Err(FeatureGraphError::PhantomSegment);
        }

        // Connectivity, open edges, and pennants, recomputed from the board.
        let reference = recompute(board);
        for component in &reference.components {
            let mut ids = component.members.iter().filter_map(|m| self.of(*m));
            let Some(first) = ids.next() else {
                return Err(FeatureGraphError::UnownedSegment);
            };
            if !ids.all(|id| id == first) {
                return Err(FeatureGraphError::ConnectedSegmentsSplit);
            }
            let Some(feature) = self.features.get(&first) else {
                return Err(FeatureGraphError::UnownedSegment);
            };
            if feature.members != component.members {
                return Err(FeatureGraphError::UnconnectedSegmentsMerged);
            }
            if feature.open_edges != component.open_edges {
                return Err(FeatureGraphError::OpenEdgeMismatch);
            }
            if feature.pennants != component.pennants {
                return Err(FeatureGraphError::PennantMismatch);
            }
        }
        if reference.components.len() != self.features.len() {
            return Err(FeatureGraphError::UnconnectedSegmentsMerged);
        }
        Ok(())
    }
}

/// One component as the from-scratch recomputation sees it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReferenceComponent {
    pub kind: FeatureKind,
    pub members: BTreeSet<SegmentRef>,
    pub open_edges: u32,
    pub pennants: u32,
}

/// The whole board's components, recomputed from nothing.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReferenceGraph {
    pub components: Vec<ReferenceComponent>,
}

/// The deliberately slow twin of [`FeatureGraph`]: walk the whole board and
/// flood-fill each component from scratch.
///
/// Structurally different from the incremental version on purpose — a
/// breadth-first search over the board versus a sequence of merges — so the two
/// do not share an idea and therefore cannot share a bug. It is production code
/// only in the sense that [`FeatureGraph::check_against`] calls it on decode;
/// nothing on the hot placement path does.
#[must_use]
pub fn recompute(board: &Board) -> ReferenceGraph {
    let mut all: BTreeSet<SegmentRef> = BTreeSet::new();
    for (coord, tile) in board.iter() {
        for index in 0..tile.segment_count() {
            if let Ok(segment) = SegmentRef::new(coord, index) {
                all.insert(segment);
            }
        }
    }

    let mut seen: BTreeSet<SegmentRef> = BTreeSet::new();
    let mut components = Vec::new();

    for start in &all {
        if seen.contains(start) {
            continue;
        }
        let Some(start_tile) = board.get(start.coord()) else {
            continue;
        };
        let Some(start_def) = start_tile.segment(start.index()) else {
            continue;
        };

        let mut members: BTreeSet<SegmentRef> = BTreeSet::new();
        let mut queue: Vec<SegmentRef> = vec![*start];
        let mut open_edges = 0u32;
        let mut pennants = 0u32;
        members.insert(*start);
        seen.insert(*start);

        while let Some(segment) = queue.pop() {
            let Some(tile) = board.get(segment.coord()) else {
                continue;
            };
            let Some(def) = tile.segment(segment.index()) else {
                continue;
            };
            pennants += u32::from(def.pennant);

            if def.kind == FeatureKind::Monastery {
                open_edges +=
                    MONASTERY_NEIGHBOURS.saturating_sub(board.surrounding_count(segment.coord()));
                continue;
            }

            for side in tile.segment_edges(segment.index()) {
                let neighbour_coord = segment.coord().neighbour(side);
                let neighbour = neighbour_coord.and_then(|coord| board.get(coord));
                let (Some(neighbour_coord), Some(neighbour)) = (neighbour_coord, neighbour) else {
                    open_edges += 1;
                    continue;
                };
                let Some(neighbour_index) = neighbour.segment_at(side.opposite()) else {
                    open_edges += 1;
                    continue;
                };
                let Ok(next) = SegmentRef::new(neighbour_coord, neighbour_index) else {
                    continue;
                };
                if seen.insert(next) {
                    members.insert(next);
                    queue.push(next);
                }
            }
        }

        components.push(ReferenceComponent {
            kind: start_def.kind,
            members,
            open_edges,
            pennants,
        });
    }

    ReferenceGraph { components }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::coord::{Rotation, Side};
    use crate::rules::tile::{PlacedTile, TileKind, START_TILE};

    fn kind_named(name: &str) -> TileKind {
        TileKind::all()
            .find(|kind| kind.def().name == name)
            .unwrap_or_else(|| panic!("no tile kind named {name}"))
    }

    /// Build a board and its graph together, so the two are always in step.
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

    fn component_at(graph: &FeatureGraph, x: i16, y: i16, index: u8) -> &Feature {
        let segment = SegmentRef::new(Coord::new(x, y).unwrap(), index).unwrap();
        let id = graph.of(segment).expect("the segment is on the board");
        graph.feature(id).expect("the id names a feature")
    }

    #[test]
    fn a_lone_start_tile_has_one_city_and_one_road_both_open() {
        let (board, graph) = build(&[(0, 0, "start", Rotation::R0)]);
        assert_eq!(graph.check_against(&board), Ok(()));
        assert_eq!(graph.len(), 2);

        let city = component_at(&graph, 0, 0, 0);
        assert_eq!(city.kind(), FeatureKind::City);
        assert_eq!(city.open_edges(), 1);
        assert!(!city.is_complete());

        let road = component_at(&graph, 0, 0, 1);
        assert_eq!(road.kind(), FeatureKind::Road);
        assert_eq!(road.open_edges(), 2);
    }

    /// Two city caps facing each other close a two-tile city.
    #[test]
    fn a_completed_feature_reports_no_open_edges() {
        // `city-cap` shows City north. Unrotated at (0,0) and turned half round
        // at (0,-1) puts the two cities edge to edge with nothing else open.
        let (board, graph) = build(&[
            (0, 0, "city-cap", Rotation::R0),
            (0, -1, "city-cap", Rotation::R180),
        ]);
        assert_eq!(graph.check_against(&board), Ok(()));

        let city = component_at(&graph, 0, 0, 0);
        assert_eq!(city.kind(), FeatureKind::City);
        assert_eq!(city.members().len(), 2);
        assert_eq!(city.tiles().len(), 2);
        assert_eq!(city.open_edges(), 0);
        assert!(city.is_complete());
        // And it is one component, not two that happen to touch.
        assert_eq!(
            graph.of(SegmentRef::new(Coord::new(0, 0).unwrap(), 0).unwrap()),
            graph.of(SegmentRef::new(Coord::new(0, -1).unwrap(), 0).unwrap())
        );
    }

    #[test]
    fn a_pennant_travels_with_the_city_it_belongs_to() {
        // `city-corner-pennant` is City{North, West}; a half turn puts its
        // cities on South and East, so its south edge joins the cap's north
        // city and its east edge stays open.
        let (board, graph) = build(&[
            (0, 0, "city-cap", Rotation::R0),
            (0, -1, "city-corner-pennant", Rotation::R180),
        ]);
        assert_eq!(graph.check_against(&board), Ok(()));
        let city = component_at(&graph, 0, 0, 0);
        assert_eq!(city.pennants(), 1);
        assert!(!city.is_complete(), "the corner still has an open edge");
    }

    /// The order the four sides are visited in must not change the result.
    /// Two placement orders that reach the same board are compared component by
    /// component, up to the ids each order happened to hand out.
    #[test]
    fn merging_is_independent_of_the_order_sides_are_processed() {
        let west_first = build(&[
            (0, 0, "road-straight", Rotation::R90),
            (-1, 0, "road-straight", Rotation::R90),
            (1, 0, "road-straight", Rotation::R90),
        ]);
        let east_first = build(&[
            (0, 0, "road-straight", Rotation::R90),
            (1, 0, "road-straight", Rotation::R90),
            (-1, 0, "road-straight", Rotation::R90),
        ]);

        for (board, graph) in [&west_first, &east_first] {
            assert_eq!(graph.check_against(board), Ok(()));
        }
        let partition = |graph: &FeatureGraph| -> Vec<(FeatureKind, BTreeSet<SegmentRef>, u32)> {
            let mut rows: Vec<_> = graph
                .iter()
                .map(|(_, feature)| {
                    (
                        feature.kind(),
                        feature.members().clone(),
                        feature.open_edges(),
                    )
                })
                .collect();
            rows.sort();
            rows
        };
        assert_eq!(partition(&west_first.1), partition(&east_first.1));
    }

    #[test]
    fn a_monastery_closes_only_when_all_eight_neighbours_are_filled() {
        let mut board = Board::new();
        let mut graph = FeatureGraph::new();
        let centre = Coord::new(0, 0).unwrap();
        board.insert(
            centre,
            PlacedTile::new(kind_named("monastery"), Rotation::R0),
        );
        graph.place_tile(&board, centre);
        assert_eq!(
            component_at(&graph, 0, 0, 0).open_edges(),
            MONASTERY_NEIGHBOURS
        );

        // Fill all eight surrounding squares with all-field monastery tiles.
        for (index, neighbour) in centre.surrounding().enumerate() {
            board.insert(
                neighbour,
                PlacedTile::new(kind_named("monastery"), Rotation::R0),
            );
            graph.place_tile(&board, neighbour);
            let expected = MONASTERY_NEIGHBOURS - u32::try_from(index + 1).unwrap();
            assert_eq!(component_at(&graph, 0, 0, 0).open_edges(), expected);
        }
        assert!(component_at(&graph, 0, 0, 0).is_complete());
        assert_eq!(graph.check_against(&board), Ok(()));
    }

    #[test]
    fn a_segment_can_only_be_claimed_once_and_a_retired_one_never() {
        let (board, mut graph) = build(&[(0, 0, "start", Rotation::R0)]);
        let city = SegmentRef::new(Coord::ORIGIN, 0).unwrap();

        assert!(graph.is_claimable(city));
        assert!(graph.place_follower(city, SeatId(0)));
        assert!(!graph.is_claimable(city), "already claimed");
        assert!(!graph.place_follower(city, SeatId(1)));
        assert_eq!(graph.check_against(&board), Ok(()));

        let id = graph.of(city).unwrap();
        let returned = graph.retire(id);
        assert_eq!(returned, BTreeMap::from([(SeatId(0), 1)]));
        assert!(graph.feature(id).unwrap().meeples().is_empty());
        assert!(!graph.is_claimable(city), "a scored feature stays claimed");
        // Retiring twice returns nothing: scoring happens exactly once.
        assert!(graph.retire(id).is_empty());
        assert_eq!(graph.check_against(&board), Ok(()));
    }

    /// A follower placed on one arm of a road that later merges with another
    /// claimed arm must survive the merge, with both owners intact.
    #[test]
    fn merging_two_claimed_components_keeps_both_owners() {
        let mut board = Board::new();
        let mut graph = FeatureGraph::new();
        let straight = kind_named("road-straight");

        let west = Coord::new(-1, 0).unwrap();
        let east = Coord::new(1, 0).unwrap();
        board.insert(west, PlacedTile::new(straight, Rotation::R90));
        graph.place_tile(&board, west);
        board.insert(east, PlacedTile::new(straight, Rotation::R90));
        graph.place_tile(&board, east);

        assert!(graph.place_follower(SegmentRef::new(west, 0).unwrap(), SeatId(0)));
        assert!(graph.place_follower(SegmentRef::new(east, 0).unwrap(), SeatId(1)));

        let middle = Coord::ORIGIN;
        board.insert(middle, PlacedTile::new(straight, Rotation::R90));
        graph.place_tile(&board, middle);

        let road = component_at(&graph, 0, 0, 0);
        assert_eq!(road.members().len(), 3);
        assert_eq!(
            road.follower_counts(),
            BTreeMap::from([(SeatId(0), 1), (SeatId(1), 1)]),
            "a merge must not lose an owner"
        );
        assert_eq!(graph.check_against(&board), Ok(()));
    }

    /// The differential in miniature: the incremental graph and the
    /// from-scratch recomputation must agree on this hand-built board. The
    /// full-match version is in `tests/features.rs`.
    #[test]
    fn the_incremental_graph_agrees_with_a_whole_board_recomputation() {
        let (board, graph) = build(&[
            (0, 0, "start", Rotation::R0),
            (0, -1, "city-cap", Rotation::R180),
            (1, 0, "road-straight", Rotation::R90),
            (2, 0, "city-cap-road", Rotation::R0),
            (0, 1, "monastery", Rotation::R0),
        ]);
        assert_eq!(graph.check_against(&board), Ok(()));

        let reference = recompute(&board);
        assert_eq!(reference.components.len(), graph.len());
        for component in &reference.components {
            let id = component
                .members
                .iter()
                .find_map(|member| graph.of(*member))
                .expect("every recomputed member is owned");
            let feature = graph.feature(id).unwrap();
            assert_eq!(feature.members(), &component.members);
            assert_eq!(feature.kind(), component.kind);
            assert_eq!(feature.open_edges(), component.open_edges);
            assert_eq!(feature.pennants(), component.pennants);
        }
    }

    #[test]
    fn a_segment_reference_rejects_an_index_no_tile_could_have() {
        assert!(SegmentRef::new(Coord::ORIGIN, MAX_SEGMENTS - 1).is_ok());
        assert_eq!(
            SegmentRef::new(Coord::ORIGIN, MAX_SEGMENTS),
            Err(SegmentRefError)
        );
        let hostile = tabula_core::canonical_encode(&RawSegmentRef {
            coord: Coord::ORIGIN,
            index: 200,
        })
        .unwrap();
        assert!(tabula_core::canonical_decode::<SegmentRef>(&hostile).is_err());
    }

    #[test]
    fn a_graph_that_forgot_a_segment_is_rejected() {
        let (board, mut graph) = build(&[(0, 0, "start", Rotation::R0)]);
        let road = SegmentRef::new(Coord::ORIGIN, 1).unwrap();
        let id = graph.of(road).unwrap();
        graph.owner.remove(&road);
        graph.features.remove(&id);
        assert_eq!(
            graph.check_against(&board),
            Err(FeatureGraphError::UnownedSegment)
        );
    }

    #[test]
    fn a_graph_with_a_wrong_open_edge_count_is_rejected() {
        let (board, mut graph) = build(&[(0, 0, "start", Rotation::R0)]);
        let city = SegmentRef::new(Coord::ORIGIN, 0).unwrap();
        let id = graph.of(city).unwrap();
        graph.features.get_mut(&id).unwrap().open_edges = 99;
        assert_eq!(
            graph.check_against(&board),
            Err(FeatureGraphError::OpenEdgeMismatch)
        );
    }

    #[test]
    fn a_graph_that_merged_two_unconnected_roads_is_rejected() {
        let straight = kind_named("road-straight");
        let mut board = Board::new();
        let mut graph = FeatureGraph::new();
        // Two roads that touch nothing of each other: a straight road running
        // north-south at the origin, and another two squares away.
        for y in [0i16, 2] {
            let coord = Coord::new(0, y).unwrap();
            board.insert(coord, PlacedTile::new(straight, Rotation::R0));
            graph.place_tile(&board, coord);
        }
        assert_eq!(graph.check_against(&board), Ok(()));

        let far = SegmentRef::new(Coord::new(0, 2).unwrap(), 0).unwrap();
        let near_id = graph
            .of(SegmentRef::new(Coord::ORIGIN, 0).unwrap())
            .unwrap();
        let far_id = graph.of(far).unwrap();
        graph.merge(near_id, far_id);
        assert!(matches!(
            graph.check_against(&board),
            Err(FeatureGraphError::UnconnectedSegmentsMerged
                | FeatureGraphError::ConnectedSegmentsSplit)
        ));
    }

    #[test]
    fn side_helper_names_stay_used() {
        // Guards against the `Side` import silently becoming dead if the tests
        // above are restructured.
        assert_eq!(Side::North.opposite(), Side::South);
    }
}
