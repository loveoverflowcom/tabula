//! Renderer-neutral focus graph and keyboard navigation service. (doc 04 §10.3)
//!
//! Games describe their focus topology as a [`FocusGraph`]; this presentation
//! service performs traversal, focus-visible tracking, and semantic action mapping.

#![allow(clippy::doc_markdown)]

use crate::{InputEvent, Key, Rect};

/// A semantic, stable identifier for a focusable node in presentation space.
///
/// Focus IDs are stable values chosen by the game or component, independent of
/// graph construction order, memory address, or render list position.
///
/// @ai.role identifier
/// @ai.domain presentation.focus
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FocusId(u32);

impl FocusId {
    /// Creates a focus identifier from a compact numerical key.
    #[must_use]
    pub const fn new(id: u32) -> Self {
        Self(id)
    }

    /// Returns the underlying raw identifier value.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

impl core::fmt::Display for FocusId {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "FocusId({})", self.0)
    }
}

impl From<u32> for FocusId {
    fn from(id: u32) -> Self {
        Self(id)
    }
}

/// The four canonical directional navigation axes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FocusDirection {
    Up,
    Down,
    Left,
    Right,
}

/// One focusable node with bounds and explicit directional adjacency.
#[derive(Clone, Debug, PartialEq)]
pub struct FocusNode {
    id: FocusId,
    rect: Rect,
    up: Option<FocusId>,
    down: Option<FocusId>,
    left: Option<FocusId>,
    right: Option<FocusId>,
}

impl FocusNode {
    /// Constructs a focus node with no directional neighbors.
    #[must_use]
    pub const fn new(id: FocusId, rect: Rect) -> Self {
        Self {
            id,
            rect,
            up: None,
            down: None,
            left: None,
            right: None,
        }
    }

    /// Constructs a focus node with explicit directional neighbors.
    #[must_use]
    pub const fn with_neighbors(
        id: FocusId,
        rect: Rect,
        up: Option<FocusId>,
        down: Option<FocusId>,
        left: Option<FocusId>,
        right: Option<FocusId>,
    ) -> Self {
        Self {
            id,
            rect,
            up,
            down,
            left,
            right,
        }
    }

    /// Returns the node's semantic identifier.
    #[must_use]
    pub const fn id(&self) -> FocusId {
        self.id
    }

    /// Returns the visual bounds of this focus node.
    #[must_use]
    pub const fn rect(&self) -> Rect {
        self.rect
    }

    /// Returns the neighbor in the given direction, if one was declared.
    #[must_use]
    pub const fn neighbor(&self, direction: FocusDirection) -> Option<FocusId> {
        match direction {
            FocusDirection::Up => self.up,
            FocusDirection::Down => self.down,
            FocusDirection::Left => self.left,
            FocusDirection::Right => self.right,
        }
    }
}

/// Structural errors rejected during [`FocusGraph`] validation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusGraphError {
    DuplicateId(FocusId),
    UnknownTarget {
        source: FocusId,
        direction: FocusDirection,
        target: FocusId,
    },
}

impl core::fmt::Display for FocusGraphError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::DuplicateId(id) => write!(formatter, "duplicate focus node identifier: {id}"),
            Self::UnknownTarget {
                source,
                direction,
                target,
            } => write!(
                formatter,
                "focus node {source} references unknown {direction:?} target {target}"
            ),
        }
    }
}

impl std::error::Error for FocusGraphError {}

/// A validated focus topology with deterministic tab order and explicit directional edges.
///
/// Games supply topology; this presentation service handles traversal.
///
/// @ai.role interaction-kernel
/// @ai.domain presentation.focus
/// @ai.pure true
/// @ai.invariant focus-never-references-missing-node
/// @ai.law deterministic-key-sequence
/// @ai.evidence crate::focus::tests::focus_graph_rejects_duplicate_ids
/// @ai.evidence crate::focus::tests::focus_graph_rejects_unknown_edge_targets
/// @ai.evidence crate::focus::tests::directional_navigation_never_leaves_the_graph
/// @ai.evidence crate::focus::tests::tab_navigation_follows_declared_stable_order
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FocusGraph {
    nodes: Vec<FocusNode>,
}

impl FocusGraph {
    /// Validates and constructs a focus graph from an ordered list of nodes.
    ///
    /// Construction verifies that all node identifiers are unique and all
    /// directional neighbor edges reference existing nodes within the graph.
    ///
    /// @ai.role proof-constructor
    /// @ai.domain presentation.focus
    /// @ai.invariant valid-focus-topology
    /// @ai.evidence crate::focus::tests::focus_graph_rejects_duplicate_ids
    /// @ai.evidence crate::focus::tests::focus_graph_rejects_unknown_edge_targets
    pub fn new(nodes: Vec<FocusNode>) -> Result<Self, FocusGraphError> {
        for index in 0..nodes.len() {
            for other_index in (index + 1)..nodes.len() {
                if nodes[index].id == nodes[other_index].id {
                    return Err(FocusGraphError::DuplicateId(nodes[index].id));
                }
            }
        }

        let contains_id = |id: FocusId| nodes.iter().any(|node| node.id == id);
        for node in &nodes {
            for (direction, neighbor) in [
                (FocusDirection::Up, node.up),
                (FocusDirection::Down, node.down),
                (FocusDirection::Left, node.left),
                (FocusDirection::Right, node.right),
            ] {
                if let Some(target) = neighbor {
                    if !contains_id(target) {
                        return Err(FocusGraphError::UnknownTarget {
                            source: node.id,
                            direction,
                            target,
                        });
                    }
                }
            }
        }

        Ok(Self { nodes })
    }

    /// Returns the slice of nodes in declared tab order.
    #[must_use]
    pub fn nodes(&self) -> &[FocusNode] {
        &self.nodes
    }

    /// Looks up a node by its identifier.
    #[must_use]
    pub fn node(&self, id: FocusId) -> Option<&FocusNode> {
        self.nodes.iter().find(|node| node.id == id)
    }

    /// Checks whether the graph contains a node with the given identifier.
    #[must_use]
    pub fn contains(&self, id: FocusId) -> bool {
        self.node(id).is_some()
    }

    /// Returns the first focus identifier in declared tab order.
    #[must_use]
    pub fn first_id(&self) -> Option<FocusId> {
        self.nodes.first().map(FocusNode::id)
    }

    /// Returns the next node in forward Tab order, cycling to the start when reaching the end.
    ///
    /// If `current` is `None` or not in the graph, selects the first node.
    #[must_use]
    pub fn next_tab(&self, current: Option<FocusId>) -> Option<FocusId> {
        if self.nodes.is_empty() {
            return None;
        }
        let Some(current) = current else {
            return self.first_id();
        };
        let current_index = self.nodes.iter().position(|node| node.id == current);
        match current_index {
            Some(index) => {
                let next_index = (index + 1) % self.nodes.len();
                Some(self.nodes[next_index].id)
            }
            None => self.first_id(),
        }
    }

    /// Traverses a directional neighbor edge from `current`.
    ///
    /// At a boundary (where no neighbor in `direction` is declared), returns `Some(current)`
    /// without wrapping. If `current` is not found in the graph, returns `None`.
    #[must_use]
    pub fn navigate(&self, current: FocusId, direction: FocusDirection) -> Option<FocusId> {
        let node = self.node(current)?;
        Some(node.neighbor(direction).unwrap_or(current))
    }
}

/// The active input modality determining focus visibility.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FocusModality {
    #[default]
    Pointer,
    Keyboard,
}

/// Ephemeral client focus state tracking the active node, modality, and window focus.
///
/// This state is presentation-local: it is never sent to canonical rules or stored in replays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FocusState {
    current: Option<FocusId>,
    modality: FocusModality,
    window_focused: bool,
}

impl Default for FocusState {
    fn default() -> Self {
        Self {
            current: None,
            modality: FocusModality::Pointer,
            window_focused: true,
        }
    }
}

impl FocusState {
    /// Constructs a focus state.
    #[must_use]
    pub const fn new(
        current: Option<FocusId>,
        modality: FocusModality,
        window_focused: bool,
    ) -> Self {
        Self {
            current,
            modality,
            window_focused,
        }
    }

    /// Returns the currently focused node identifier, if any.
    #[must_use]
    pub const fn current(&self) -> Option<FocusId> {
        self.current
    }

    /// Returns the active input modality.
    #[must_use]
    pub const fn modality(&self) -> FocusModality {
        self.modality
    }

    /// Returns whether the presentation window currently holds OS/platform focus.
    #[must_use]
    pub const fn is_window_focused(&self) -> bool {
        self.window_focused
    }

    /// Returns whether a semantic focus ring should be rendered.
    ///
    /// True when the window is focused, keyboard modality is active, and a node is focused.
    #[must_use]
    pub const fn is_focus_visible(&self) -> bool {
        self.window_focused
            && matches!(self.modality, FocusModality::Keyboard)
            && self.current.is_some()
    }

    /// Sets the focused node without changing the modality.
    pub fn set_current(&mut self, current: Option<FocusId>) {
        self.current = current;
    }

    /// Updates focus from a pointer interaction, setting modality to [`FocusModality::Pointer`].
    pub fn set_pointer_focus(&mut self, current: Option<FocusId>) {
        self.current = current;
        self.modality = FocusModality::Pointer;
    }

    /// Updates focus from a keyboard interaction, setting modality to [`FocusModality::Keyboard`].
    pub fn set_keyboard_focus(&mut self, current: Option<FocusId>) {
        self.current = current;
        self.modality = FocusModality::Keyboard;
    }

    /// Updates window focus status.
    pub fn set_window_focused(&mut self, window_focused: bool) {
        self.window_focused = window_focused;
    }

    /// Reconciles focus against a new focus graph.
    ///
    /// If the currently focused node is absent from `graph`, clears focus.
    pub fn reconcile(&mut self, graph: &FocusGraph) {
        if let Some(id) = self.current {
            if !graph.contains(id) {
                self.current = None;
            }
        }
    }
}

/// The domain-neutral outcome of processing an input event against a focus graph.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavigationAction {
    /// No focus change or activation occurred.
    None,
    /// Logical focus moved to another node.
    FocusChanged(FocusId),
    /// The currently focused node was activated (e.g. Enter or Space).
    Activate(FocusId),
    /// The user requested cancellation (e.g. Escape).
    Cancel,
}

/// Evaluates normalized input against a focus graph and updates focus state.
///
/// This function is pure with respect to domain commands: it emits only generic
/// navigation results. The game maps [`NavigationAction::Activate`] to its own commands.
///
/// @ai.role interaction-service
/// @ai.domain presentation.focus
/// @ai.pure true
/// @ai.law key-release-does-not-move-or-activate-focus
/// @ai.law window-focus-loss-never-activates-a-node
/// @ai.evidence crate::focus::tests::key_release_does_not_move_or_activate_focus
/// @ai.evidence crate::focus::tests::window_focus_loss_never_activates_a_node
/// @ai.evidence crate::focus::tests::escape_produces_generic_cancel_action
#[allow(clippy::doc_markdown)]
pub fn handle_navigation(
    graph: &FocusGraph,
    state: &mut FocusState,
    event: &InputEvent,
) -> NavigationAction {
    match event {
        InputEvent::Key { key, pressed: true } => match key {
            Key::ArrowUp => step_directional(graph, state, FocusDirection::Up),
            Key::ArrowDown => step_directional(graph, state, FocusDirection::Down),
            Key::ArrowLeft => step_directional(graph, state, FocusDirection::Left),
            Key::ArrowRight => step_directional(graph, state, FocusDirection::Right),
            Key::Tab => {
                state.modality = FocusModality::Keyboard;
                let next = graph.next_tab(state.current);
                state.current = next;
                next.map_or(NavigationAction::None, NavigationAction::FocusChanged)
            }
            Key::Enter | Key::Space => {
                state.modality = FocusModality::Keyboard;
                if let Some(id) = state.current {
                    if graph.contains(id) {
                        return NavigationAction::Activate(id);
                    }
                }
                NavigationAction::None
            }
            Key::Escape => NavigationAction::Cancel,
        },
        InputEvent::Key { pressed: false, .. } | InputEvent::Pointer { .. } => {
            NavigationAction::None
        }
        InputEvent::Focus(focused) => {
            state.window_focused = *focused;
            NavigationAction::None
        }
    }
}

fn step_directional(
    graph: &FocusGraph,
    state: &mut FocusState,
    direction: FocusDirection,
) -> NavigationAction {
    state.modality = FocusModality::Keyboard;
    let target = state
        .current
        .map_or_else(|| graph.first_id(), |curr| graph.navigate(curr, direction));

    if target == state.current {
        NavigationAction::None
    } else {
        state.current = target;
        target.map_or(NavigationAction::None, NavigationAction::FocusChanged)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec2;

    fn rect(x: f32, y: f32) -> Rect {
        Rect::new(Vec2::new(x, y), Vec2::splat(10.0)).expect("test rect is valid")
    }

    fn sample_graph() -> FocusGraph {
        // Topology:
        // A(1) — B(2) — C(3)
        //          |
        //         D(4)
        let a = FocusNode::with_neighbors(
            FocusId::new(1),
            rect(0.0, 0.0),
            None,
            None,
            None,
            Some(FocusId::new(2)),
        );
        let b = FocusNode::with_neighbors(
            FocusId::new(2),
            rect(10.0, 0.0),
            None,
            Some(FocusId::new(4)),
            Some(FocusId::new(1)),
            Some(FocusId::new(3)),
        );
        let c = FocusNode::with_neighbors(
            FocusId::new(3),
            rect(20.0, 0.0),
            None,
            None,
            Some(FocusId::new(2)),
            None,
        );
        let d = FocusNode::with_neighbors(
            FocusId::new(4),
            rect(10.0, 10.0),
            Some(FocusId::new(2)),
            None,
            None,
            None,
        );
        FocusGraph::new(vec![a, b, c, d]).expect("sample graph is valid")
    }

    #[test]
    fn focus_graph_rejects_duplicate_ids() {
        let n1 = FocusNode::new(FocusId::new(1), rect(0.0, 0.0));
        let n2 = FocusNode::new(FocusId::new(1), rect(10.0, 0.0));
        assert_eq!(
            FocusGraph::new(vec![n1, n2]),
            Err(FocusGraphError::DuplicateId(FocusId::new(1)))
        );
    }

    #[test]
    fn focus_graph_rejects_unknown_edge_targets() {
        let n1 = FocusNode::with_neighbors(
            FocusId::new(1),
            rect(0.0, 0.0),
            None,
            None,
            None,
            Some(FocusId::new(99)),
        );
        assert_eq!(
            FocusGraph::new(vec![n1]),
            Err(FocusGraphError::UnknownTarget {
                source: FocusId::new(1),
                direction: FocusDirection::Right,
                target: FocusId::new(99),
            })
        );
    }

    #[test]
    fn directional_navigation_never_leaves_the_graph() {
        let graph = sample_graph();
        // A -> Right -> B
        assert_eq!(
            graph.navigate(FocusId::new(1), FocusDirection::Right),
            Some(FocusId::new(2))
        );
        // B -> Down -> D
        assert_eq!(
            graph.navigate(FocusId::new(2), FocusDirection::Down),
            Some(FocusId::new(4))
        );
        // D -> Up -> B
        assert_eq!(
            graph.navigate(FocusId::new(4), FocusDirection::Up),
            Some(FocusId::new(2))
        );
        // B -> Right -> C
        assert_eq!(
            graph.navigate(FocusId::new(2), FocusDirection::Right),
            Some(FocusId::new(3))
        );
        // C -> Left -> B
        assert_eq!(
            graph.navigate(FocusId::new(3), FocusDirection::Left),
            Some(FocusId::new(2))
        );

        // Boundaries: no wrapping
        assert_eq!(
            graph.navigate(FocusId::new(1), FocusDirection::Left),
            Some(FocusId::new(1))
        );
        assert_eq!(
            graph.navigate(FocusId::new(1), FocusDirection::Up),
            Some(FocusId::new(1))
        );
        assert_eq!(
            graph.navigate(FocusId::new(3), FocusDirection::Right),
            Some(FocusId::new(3))
        );
        assert_eq!(
            graph.navigate(FocusId::new(4), FocusDirection::Down),
            Some(FocusId::new(4))
        );
    }

    #[test]
    fn tab_navigation_follows_declared_stable_order() {
        let graph = sample_graph();
        assert_eq!(graph.next_tab(None), Some(FocusId::new(1)));
        assert_eq!(graph.next_tab(Some(FocusId::new(1))), Some(FocusId::new(2)));
        assert_eq!(graph.next_tab(Some(FocusId::new(2))), Some(FocusId::new(3)));
        assert_eq!(graph.next_tab(Some(FocusId::new(3))), Some(FocusId::new(4)));
        // Cycles back to start
        assert_eq!(graph.next_tab(Some(FocusId::new(4))), Some(FocusId::new(1)));
        // Unknown id recovers to first
        assert_eq!(
            graph.next_tab(Some(FocusId::new(99))),
            Some(FocusId::new(1))
        );
    }

    #[test]
    fn focus_state_reconcile_clears_missing_focus() {
        let graph = sample_graph();
        let mut state = FocusState::new(Some(FocusId::new(2)), FocusModality::Keyboard, true);
        state.reconcile(&graph);
        assert_eq!(state.current(), Some(FocusId::new(2)));

        state.set_current(Some(FocusId::new(99)));
        state.reconcile(&graph);
        assert_eq!(state.current(), None);
    }

    #[test]
    fn key_release_does_not_move_or_activate_focus() {
        let graph = sample_graph();
        let mut state = FocusState::new(Some(FocusId::new(1)), FocusModality::Keyboard, true);
        let event = InputEvent::Key {
            key: Key::Enter,
            pressed: false,
        };
        assert_eq!(
            handle_navigation(&graph, &mut state, &event),
            NavigationAction::None
        );
        assert_eq!(state.current(), Some(FocusId::new(1)));
    }

    #[test]
    fn window_focus_loss_never_activates_a_node() {
        let graph = sample_graph();
        let mut state = FocusState::new(Some(FocusId::new(1)), FocusModality::Keyboard, true);
        let event = InputEvent::Focus(false);
        assert_eq!(
            handle_navigation(&graph, &mut state, &event),
            NavigationAction::None
        );
        assert!(!state.is_window_focused());
        assert!(!state.is_focus_visible());
    }

    #[test]
    fn enter_and_space_activate_the_current_focus_node() {
        let graph = sample_graph();
        let mut state = FocusState::new(Some(FocusId::new(2)), FocusModality::Keyboard, true);
        assert_eq!(
            handle_navigation(
                &graph,
                &mut state,
                &InputEvent::Key {
                    key: Key::Enter,
                    pressed: true
                }
            ),
            NavigationAction::Activate(FocusId::new(2))
        );
        assert_eq!(
            handle_navigation(
                &graph,
                &mut state,
                &InputEvent::Key {
                    key: Key::Space,
                    pressed: true
                }
            ),
            NavigationAction::Activate(FocusId::new(2))
        );
    }

    #[test]
    fn escape_produces_generic_cancel_action() {
        let graph = sample_graph();
        let mut state = FocusState::new(Some(FocusId::new(2)), FocusModality::Keyboard, true);
        assert_eq!(
            handle_navigation(
                &graph,
                &mut state,
                &InputEvent::Key {
                    key: Key::Escape,
                    pressed: true
                }
            ),
            NavigationAction::Cancel
        );
    }

    #[test]
    fn keyboard_navigation_activates_keyboard_modality_and_focus_visible() {
        let graph = sample_graph();
        let mut state = FocusState::new(Some(FocusId::new(1)), FocusModality::Pointer, true);
        assert!(!state.is_focus_visible());

        let action = handle_navigation(
            &graph,
            &mut state,
            &InputEvent::Key {
                key: Key::ArrowRight,
                pressed: true,
            },
        );
        assert_eq!(action, NavigationAction::FocusChanged(FocusId::new(2)));
        assert_eq!(state.modality(), FocusModality::Keyboard);
        assert!(state.is_focus_visible());
    }
}
