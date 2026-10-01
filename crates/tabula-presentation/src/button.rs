//! Small, labeled gameplay actions built from existing render commands. (doc 04 §5, §10)
//!
//! Games own action meaning and focus topology. This helper owns only local input
//! mechanics and semantic appearance; it never constructs a game command (I-10).

#![allow(clippy::float_arithmetic)]

use tabula_design::{Color, Percent, Positive, Theme};

use crate::{
    handle_navigation, Align, Border, Corners, FocusDirection, FocusGraph, FocusId, FocusNode,
    FocusState, InputEvent, Key, Layer, NavigationAction, Opacity, Paint, PointerButton,
    PointerPhase, Rect, RenderCmd, RenderListBuilder, RenderListError, TextStyleToken, Vec2,
};

/// Semantic emphasis for a labeled action. (doc 04 §7–§8)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ButtonTone {
    /// Principal action or selected option, using primary/on-primary.
    Filled,
    /// Related action, using container-high/on-surface.
    #[default]
    Tonal,
}

/// Shape of an action or a horizontal connected group. (doc 04 §8)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ButtonShape {
    #[default]
    Round,
    Square,
    ConnectedStart,
    ConnectedMiddle,
    ConnectedEnd,
}

/// One renderer-neutral, labeled action with fixed logical hit/focus bounds.
///
/// The consumer maps its stable identifier to an intent and supplies a focus graph.
/// Disabled/busy actions use `enabled(false)`; their reason belongs in nearby text.
/// This is a small gameplay primitive, not a shell component framework (doc 04 §5.4).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ActionButton<'a> {
    id: FocusId,
    rect: Rect,
    label: &'a str,
    icon: Option<&'a str>,
    tone: ButtonTone,
    shape: ButtonShape,
    enabled: bool,
}

impl<'a> ActionButton<'a> {
    /// Validates the supplied bounds against both the theme target and the 44 dp floor.
    ///
    /// Bounds are never enlarged implicitly: rendering, focus, and pointer input
    /// share this exact rectangle, even while the pressed shape changes.
    pub fn new(
        id: FocusId,
        rect: Rect,
        label: &'a str,
        min_target: Positive,
    ) -> Result<Self, RenderListError> {
        let minimum = min_target.get().max(44.0);
        if rect.size().x < minimum || rect.size().y < minimum {
            return Err(RenderListError::InvalidGeometry);
        }
        Ok(Self {
            id,
            rect,
            label,
            icon: None,
            tone: ButtonTone::Tonal,
            shape: ButtonShape::Round,
            enabled: true,
        })
    }

    /// Adds a glyph above the retained action label.
    #[must_use]
    pub const fn with_icon(mut self, icon: &'a str) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Selects semantic action emphasis.
    #[must_use]
    pub const fn tone(mut self, tone: ButtonTone) -> Self {
        self.tone = tone;
        self
    }

    /// Selects standalone or connected-group shape.
    #[must_use]
    pub const fn shape(mut self, shape: ButtonShape) -> Self {
        self.shape = shape;
        self
    }

    /// Enables activation, or applies disabled visuals and suppresses input.
    #[must_use]
    pub const fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Returns the consumer's stable action/focus identifier.
    #[must_use]
    pub const fn id(self) -> FocusId {
        self.id
    }

    /// Returns the fixed logical hit and focus bounds.
    #[must_use]
    pub const fn rect(self) -> Rect {
        self.rect
    }

    /// Returns whether this action accepts input.
    #[must_use]
    pub const fn is_enabled(self) -> bool {
        self.enabled
    }

    /// Emits semantic fill, state feedback, label, and keyboard-visible focus.
    ///
    /// Feedback is immediate, including reduced motion. No clock or animation
    /// timeline enters this helper, and press feedback never changes hit geometry.
    /// Equal ordering keys preserve insertion order inside the caller's layer.
    pub fn draw(
        self,
        builder: &mut RenderListBuilder,
        theme: &Theme,
        interaction: &ButtonInteraction,
        focus: &FocusState,
        layer: Layer,
    ) -> Result<(), RenderListError> {
        let pressed = self.enabled && interaction.pressed() == Some(self.id);
        let focused = self.enabled && focus.is_focus_visible() && focus.current() == Some(self.id);
        let radii = self.radii(theme, pressed)?;
        if focused {
            // Exterior strokes preserve a surface gap beside primary-filled
            // actions. Draw above every sibling fill so connected options cannot
            // obscure the ring; no filled focus rectangle covers action content.
            let gap = f32::from(theme.space.xxs);
            for (offset, width, color) in [
                (
                    gap + theme.focus.ring_width.get() / 2.0,
                    theme.focus.ring_width.get(),
                    theme.focus.ring_color,
                ),
                (gap / 2.0, gap, theme.color.surface),
            ] {
                builder.push(RenderCmd::Rect {
                    rect: expanded(self.rect, offset)?,
                    radii: expanded_corners(radii, offset)?,
                    fill: None,
                    border: Some(Border::new(width, color)?),
                    layer,
                    z: 1,
                })?;
            }
        }

        let (container, content) = if self.enabled {
            match self.tone {
                ButtonTone::Filled => (theme.color.primary, theme.color.on_primary),
                ButtonTone::Tonal => (theme.color.surface_container_high, theme.color.on_surface),
            }
        } else {
            (theme.color.on_surface, theme.color.on_surface)
        };
        Self::draw_rect(
            builder,
            self.rect,
            radii,
            container,
            (!self.enabled).then_some(theme.state.disabled_container),
            layer,
        )?;
        let state_alpha = if pressed {
            Some(theme.state.press)
        } else if focused {
            Some(theme.state.focus)
        } else if self.enabled && interaction.hovered() == Some(self.id) {
            Some(theme.state.hover)
        } else {
            None
        };
        if let Some(state_alpha) = state_alpha {
            Self::draw_rect(builder, self.rect, radii, content, Some(state_alpha), layer)?;
        }
        self.draw_label(builder, theme, content, layer)
    }

    fn radii(self, theme: &Theme, pressed: bool) -> Result<Corners, RenderListError> {
        let limit = self.rect.size().min_element() / 2.0;
        let round = theme.shape.full.get().min(limit);
        let square = theme.shape.button.get().min(limit);
        let inner = if pressed {
            square
        } else {
            theme.shape.sm.get().min(limit)
        };
        match self.shape {
            ButtonShape::Round => Corners::uniform(if pressed { square } else { round }),
            ButtonShape::Square => Corners::uniform(square),
            ButtonShape::ConnectedStart => Corners::new(round, inner, inner, round),
            ButtonShape::ConnectedMiddle => Corners::uniform(inner),
            ButtonShape::ConnectedEnd => Corners::new(inner, round, round, inner),
        }
    }

    fn draw_rect(
        builder: &mut RenderListBuilder,
        rect: Rect,
        radii: Corners,
        fill: Color,
        opacity: Option<Percent>,
        layer: Layer,
    ) -> Result<(), RenderListError> {
        if let Some(opacity) = opacity {
            push_opacity(builder, opacity, layer)?;
        }
        builder.push(RenderCmd::Rect {
            rect,
            radii,
            fill: Some(Paint::Solid(fill)),
            border: None,
            layer,
            z: 0,
        })?;
        if opacity.is_some() {
            builder.push(RenderCmd::PopOpacity { layer, z: 0 })?;
        }
        Ok(())
    }

    fn draw_label(
        self,
        builder: &mut RenderListBuilder,
        theme: &Theme,
        color: Color,
        layer: Layer,
    ) -> Result<(), RenderListError> {
        let label_style = if self.icon.is_some() {
            TextStyleToken::LabelMd
        } else {
            TextStyleToken::LabelLg
        };
        let label_height = theme.text_style(label_style).line_height().get();
        let center_x = self.rect.origin().x + self.rect.size().x / 2.0;
        let max_width = Positive::new(self.rect.size().x - f32::from(theme.space.sm) * 2.0)
            .map_err(|_| RenderListError::InvalidGeometry)?;
        let mut label_y = self.rect.origin().y + (self.rect.size().y - label_height) / 2.0;
        if !self.enabled {
            push_opacity(builder, theme.state.disabled_content, layer)?;
        }
        if let Some(icon) = self.icon {
            let large_height = theme
                .text_style(TextStyleToken::TitleLg)
                .line_height()
                .get();
            let gap = f32::from(theme.space.xxs);
            let icon_style = if large_height + label_height + gap <= self.rect.size().y {
                TextStyleToken::TitleLg
            } else {
                TextStyleToken::TitleMd
            };
            let icon_height = theme.text_style(icon_style).line_height().get();
            let icon_y = self.rect.origin().y
                + (self.rect.size().y - icon_height - gap - label_height) / 2.0;
            builder.push(RenderCmd::Text {
                text: icon.into(),
                at: Vec2::new(center_x, icon_y),
                style: icon_style,
                align: Align::Center,
                max_width: Some(max_width),
                color,
                layer,
                z: 0,
            })?;
            label_y = icon_y + icon_height + gap;
        }
        builder.push(RenderCmd::Text {
            text: self.label.into(),
            at: Vec2::new(center_x, label_y),
            style: label_style,
            align: Align::Center,
            max_width: Some(max_width),
            color,
            layer,
            z: 0,
        })?;
        if !self.enabled {
            builder.push(RenderCmd::PopOpacity { layer, z: 0 })?;
        }
        Ok(())
    }
}

/// Local interaction state for a small action group. (doc 04 §10, I-10)
///
/// A primary pointer must press and release the same enabled target without
/// leaving it. Enter/Space activate once per physical press. Missing/disabled
/// actions, cancellation, and window focus loss discard pending pointer presses.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ButtonInteraction {
    hovered: Option<FocusId>,
    pointer_pressed: Option<FocusId>,
    keyboard_pressed: Option<(Key, FocusId)>,
    enter_held: bool,
    space_held: bool,
}

impl ButtonInteraction {
    /// Returns the enabled action under the primary pointer.
    #[must_use]
    pub const fn hovered(&self) -> Option<FocusId> {
        self.hovered
    }

    /// Returns the action currently showing press feedback.
    #[must_use]
    pub fn pressed(&self) -> Option<FocusId> {
        self.pointer_pressed
            .or(self.keyboard_pressed.map(|(_, id)| id))
    }

    /// Suppresses an activation key already held when a consumer opens an overlay.
    ///
    /// Opening with Enter must not activate the new overlay's first action when
    /// the backend repeats that same keydown. Non-activation keys are ignored.
    pub fn suppress_activation_until_release(&mut self, key: Key) {
        match key {
            Key::Enter => self.enter_held = true,
            Key::Space => self.space_held = true,
            _ => {}
        }
    }

    /// Evaluates normalized input and returns generic focus/activation only.
    ///
    /// Disabled and missing controls are removed from traversal. The supplied
    /// graph retains its declared order and follows directional edges past
    /// disabled controls. The consumer maps activation to its own intent.
    pub fn on_input(
        &mut self,
        input: &InputEvent,
        buttons: &[ActionButton<'_>],
        graph: &FocusGraph,
        focus: &mut FocusState,
    ) -> NavigationAction {
        let graph = enabled_graph(buttons, graph);
        focus.reconcile(&graph);
        self.hovered = self.hovered.filter(|id| graph.contains(*id));
        self.pointer_pressed = self.pointer_pressed.filter(|id| graph.contains(*id));
        self.keyboard_pressed = self
            .keyboard_pressed
            .filter(|(_, id)| graph.contains(*id) && focus.current() == Some(*id));

        match input {
            InputEvent::Focus(focused) => {
                focus.set_window_focused(*focused);
                if !focused {
                    self.clear_pending();
                    self.hovered = None;
                }
                NavigationAction::None
            }
            InputEvent::Key { key, pressed } if matches!(key, Key::Enter | Key::Space) => {
                let held = match key {
                    Key::Enter => &mut self.enter_held,
                    _ => &mut self.space_held,
                };
                let repeated = *held;
                *held = *pressed;
                if !pressed {
                    if self
                        .keyboard_pressed
                        .is_some_and(|(active, _)| active == *key)
                    {
                        self.keyboard_pressed = None;
                    }
                    return NavigationAction::None;
                }
                if repeated || !focus.is_window_focused() {
                    return NavigationAction::None;
                }
                self.pointer_pressed = None;
                let action = handle_navigation(&graph, focus, input);
                if let NavigationAction::Activate(id) = action {
                    self.keyboard_pressed = Some((*key, id));
                }
                action
            }
            _ if !focus.is_window_focused() => NavigationAction::None,
            InputEvent::Key { .. } => {
                let action = handle_navigation(&graph, focus, input);
                if !matches!(action, NavigationAction::None) {
                    self.clear_pending();
                }
                action
            }
            InputEvent::Pointer {
                position,
                button: PointerButton::Primary,
                phase,
            } => {
                let hit = buttons.iter().find(|button| {
                    button.enabled
                        && graph.contains(button.id)
                        && button.rect.contains(position.get())
                });
                self.hovered = hit.map(|button| button.id);
                match phase {
                    PointerPhase::Down => {
                        self.keyboard_pressed = None;
                        self.pointer_pressed = self.hovered;
                        focus.set_pointer_focus(self.hovered);
                        self.hovered
                            .map_or(NavigationAction::None, NavigationAction::FocusChanged)
                    }
                    PointerPhase::Move => {
                        if self.pointer_pressed != self.hovered {
                            self.pointer_pressed = None;
                        }
                        NavigationAction::None
                    }
                    PointerPhase::Up => {
                        let pressed = self.pointer_pressed.take();
                        if pressed.is_some() && pressed == self.hovered {
                            pressed.map_or(NavigationAction::None, NavigationAction::Activate)
                        } else {
                            NavigationAction::None
                        }
                    }
                    PointerPhase::Cancel => {
                        self.pointer_pressed = None;
                        self.hovered = None;
                        NavigationAction::None
                    }
                }
            }
            InputEvent::Pointer { .. } => NavigationAction::None,
        }
    }

    fn clear_pending(&mut self) {
        self.pointer_pressed = None;
        self.keyboard_pressed = None;
    }
}

fn enabled_graph(buttons: &[ActionButton<'_>], graph: &FocusGraph) -> FocusGraph {
    let enabled = |id| {
        buttons
            .iter()
            .any(|button| button.id == id && button.enabled)
    };
    let neighbor = |source, direction| {
        let mut current = source;
        for _ in 0..graph.nodes().len() {
            let next = graph.node(current)?.neighbor(direction)?;
            if enabled(next) {
                return Some(next);
            }
            current = next;
        }
        None
    };
    let nodes = graph
        .nodes()
        .iter()
        .filter(|node| enabled(node.id()))
        .map(|node| {
            FocusNode::with_neighbors(
                node.id(),
                node.rect(),
                neighbor(node.id(), FocusDirection::Up),
                neighbor(node.id(), FocusDirection::Down),
                neighbor(node.id(), FocusDirection::Left),
                neighbor(node.id(), FocusDirection::Right),
            )
        })
        .collect();
    FocusGraph::new(nodes).expect("filtering a validated graph retains valid edges and unique ids")
}

fn push_opacity(
    builder: &mut RenderListBuilder,
    percent: Percent,
    layer: Layer,
) -> Result<(), RenderListError> {
    builder.push(RenderCmd::PushOpacity {
        opacity: Opacity::try_from(f32::from(percent.get()) / 100.0)
            .expect("validated percentage is within opacity bounds"),
        layer,
        z: 0,
    })
}

fn expanded(rect: Rect, offset: f32) -> Result<Rect, RenderListError> {
    Rect::new(
        rect.origin() - Vec2::splat(offset),
        rect.size() + Vec2::splat(offset * 2.0),
    )
}

fn expanded_corners(radii: Corners, offset: f32) -> Result<Corners, RenderListError> {
    Corners::new(
        radii.top_left() + offset,
        radii.top_right() + offset,
        radii.bottom_right() + offset,
        radii.bottom_left() + offset,
    )
}

#[cfg(test)]
mod tests {
    // These token values and half/integer coordinates are exactly representable.
    #![allow(clippy::float_cmp)]

    use super::*;
    use crate::{Camera2D, Dpi, FocusModality, FrameCtx, PointerPosition, RenderList, Viewport};
    use tabula_design::ThemeKind;

    const FIRST: FocusId = FocusId::new(1);
    const SECOND: FocusId = FocusId::new(2);
    const THIRD: FocusId = FocusId::new(3);

    fn theme() -> Theme {
        Theme::by_kind(ThemeKind::Light)
    }

    fn button(id: FocusId, x: f32) -> ActionButton<'static> {
        ActionButton::new(
            id,
            Rect::new(Vec2::new(x, 10.0), Vec2::splat(60.0)).unwrap(),
            "Choose",
            theme().density.min_target,
        )
        .unwrap()
    }

    fn buttons() -> [ActionButton<'static>; 3] {
        [
            button(FIRST, 10.0),
            button(SECOND, 72.0),
            button(THIRD, 134.0),
        ]
    }

    fn graph(buttons: &[ActionButton<'_>]) -> FocusGraph {
        FocusGraph::new(
            buttons
                .iter()
                .enumerate()
                .map(|(index, button)| {
                    FocusNode::with_neighbors(
                        button.id,
                        button.rect,
                        None,
                        None,
                        index.checked_sub(1).map(|index| buttons[index].id),
                        buttons.get(index + 1).map(|button| button.id),
                    )
                })
                .collect(),
        )
        .unwrap()
    }

    fn pointer(x: f32, phase: PointerPhase) -> InputEvent {
        InputEvent::Pointer {
            position: PointerPosition::new(Vec2::new(x, 30.0)).unwrap(),
            button: PointerButton::Primary,
            phase,
        }
    }

    fn key(key: Key, pressed: bool) -> InputEvent {
        InputEvent::Key { key, pressed }
    }

    fn draw(
        button: ActionButton<'_>,
        theme: &Theme,
        interaction: &ButtonInteraction,
        focus: &FocusState,
    ) -> RenderList {
        let mut builder = RenderListBuilder::new(Camera2D::default());
        button
            .draw(&mut builder, theme, interaction, focus, Layer::MODAL)
            .unwrap();
        builder.finish().unwrap()
    }

    #[test]
    fn action_bounds_obey_theme_target_and_never_fall_below_44_dp() {
        for (minimum, size, accepted) in [
            (44.0, Vec2::splat(44.0), true),
            (44.0, Vec2::new(43.99, 60.0), false),
            (44.0, Vec2::new(60.0, 43.99), false),
            (60.0, Vec2::splat(60.0), true),
            (60.0, Vec2::new(60.0, 59.99), false),
            (1.0, Vec2::splat(43.0), false),
        ] {
            let result = ActionButton::new(
                FIRST,
                Rect::new(Vec2::ZERO, size).unwrap(),
                "Choose",
                Positive::new(minimum).unwrap(),
            );
            assert_eq!(result.is_ok(), accepted, "minimum {minimum}, size {size:?}");
        }
    }

    #[test]
    fn minimum_target_edges_remain_hittable_and_connected_corners_follow_group_position() {
        let button = ActionButton::new(
            FIRST,
            Rect::new(Vec2::ZERO, Vec2::splat(44.0)).unwrap(),
            "Choose",
            theme().density.min_target,
        )
        .unwrap();
        let buttons = [button];
        let graph = graph(&buttons);
        for position in [
            Vec2::ZERO,
            Vec2::new(44.0, 0.0),
            Vec2::new(0.0, 44.0),
            Vec2::splat(44.0),
        ] {
            let mut interaction = ButtonInteraction::default();
            let mut focus = FocusState::default();
            for phase in [PointerPhase::Down, PointerPhase::Up] {
                let action = interaction.on_input(
                    &InputEvent::Pointer {
                        position: PointerPosition::new(position).unwrap(),
                        button: PointerButton::Primary,
                        phase,
                    },
                    &buttons,
                    &graph,
                    &mut focus,
                );
                assert_eq!(
                    action,
                    match phase {
                        PointerPhase::Down => NavigationAction::FocusChanged(FIRST),
                        _ => NavigationAction::Activate(FIRST),
                    }
                );
            }
        }
        for (shape, expected) in [
            (ButtonShape::Square, [14.0; 4]),
            (ButtonShape::ConnectedStart, [22.0, 8.0, 8.0, 22.0]),
            (ButtonShape::ConnectedMiddle, [8.0; 4]),
            (ButtonShape::ConnectedEnd, [8.0, 22.0, 22.0, 8.0]),
        ] {
            let list = draw(
                button.shape(shape),
                &theme(),
                &ButtonInteraction::default(),
                &FocusState::default(),
            );
            let RenderCmd::Rect { radii, .. } = list.commands()[0] else {
                panic!("button fill expected")
            };
            assert_eq!(
                [
                    radii.top_left(),
                    radii.top_right(),
                    radii.bottom_right(),
                    radii.bottom_left()
                ],
                expected
            );
        }
        let mut interaction = ButtonInteraction::default();
        let mut focus = FocusState::default();
        interaction.on_input(
            &pointer(30.0, PointerPhase::Down),
            &buttons,
            &graph,
            &mut focus,
        );
        for (shape, expected) in [
            (ButtonShape::ConnectedStart, [22.0, 14.0, 14.0, 22.0]),
            (ButtonShape::ConnectedMiddle, [14.0; 4]),
            (ButtonShape::ConnectedEnd, [14.0, 22.0, 22.0, 14.0]),
        ] {
            let list = draw(button.shape(shape), &theme(), &interaction, &focus);
            let RenderCmd::Rect { rect, radii, .. } = list.commands()[0] else {
                panic!("button fill expected")
            };
            assert_eq!(rect, button.rect());
            assert_eq!(
                [
                    radii.top_left(),
                    radii.top_right(),
                    radii.bottom_right(),
                    radii.bottom_left()
                ],
                expected
            );
        }
    }

    #[test]
    fn primary_pointer_requires_press_and_release_on_the_same_target() {
        let buttons = buttons();
        let graph = graph(&buttons);
        let mut interaction = ButtonInteraction::default();
        let mut focus = FocusState::default();
        assert_eq!(
            interaction.on_input(
                &pointer(30.0, PointerPhase::Up),
                &buttons,
                &graph,
                &mut focus
            ),
            NavigationAction::None
        );
        assert_eq!(
            interaction.on_input(
                &pointer(30.0, PointerPhase::Down),
                &buttons,
                &graph,
                &mut focus
            ),
            NavigationAction::FocusChanged(FIRST)
        );
        assert_eq!(focus.modality(), FocusModality::Pointer);
        assert_eq!(interaction.pressed(), Some(FIRST));
        assert_eq!(
            interaction.on_input(
                &pointer(30.0, PointerPhase::Up),
                &buttons,
                &graph,
                &mut focus
            ),
            NavigationAction::Activate(FIRST)
        );
        assert_eq!(interaction.pressed(), None);
        interaction.on_input(
            &pointer(30.0, PointerPhase::Down),
            &buttons,
            &graph,
            &mut focus,
        );
        assert_eq!(
            interaction.on_input(
                &pointer(90.0, PointerPhase::Up),
                &buttons,
                &graph,
                &mut focus
            ),
            NavigationAction::None
        );
    }

    #[test]
    fn pointer_cancellation_leaving_or_focus_loss_discards_the_press() {
        let buttons = buttons();
        let graph = graph(&buttons);
        for interrupted in [
            pointer(30.0, PointerPhase::Cancel),
            pointer(300.0, PointerPhase::Move),
            InputEvent::Focus(false),
            key(Key::Escape, true),
        ] {
            let mut interaction = ButtonInteraction::default();
            let mut focus = FocusState::default();
            interaction.on_input(
                &pointer(30.0, PointerPhase::Down),
                &buttons,
                &graph,
                &mut focus,
            );
            interaction.on_input(&interrupted, &buttons, &graph, &mut focus);
            interaction.on_input(&InputEvent::Focus(true), &buttons, &graph, &mut focus);
            // Returning into the old target never resurrects a canceled press.
            interaction.on_input(
                &pointer(30.0, PointerPhase::Move),
                &buttons,
                &graph,
                &mut focus,
            );
            assert_eq!(
                interaction.on_input(
                    &pointer(30.0, PointerPhase::Up),
                    &buttons,
                    &graph,
                    &mut focus
                ),
                NavigationAction::None,
                "interrupted by {interrupted:?}"
            );
        }
    }

    #[test]
    fn secondary_and_middle_pointer_buttons_never_activate() {
        let buttons = buttons();
        let graph = graph(&buttons);
        for button in [PointerButton::Secondary, PointerButton::Middle] {
            let mut interaction = ButtonInteraction::default();
            let mut focus = FocusState::default();
            for phase in [PointerPhase::Down, PointerPhase::Up] {
                let input = InputEvent::Pointer {
                    position: PointerPosition::new(Vec2::new(30.0, 30.0)).unwrap(),
                    button,
                    phase,
                };
                assert_eq!(
                    interaction.on_input(&input, &buttons, &graph, &mut focus),
                    NavigationAction::None
                );
            }
            assert_eq!(focus.current(), None);
        }
    }

    #[test]
    fn disabling_or_removing_a_pressed_action_prevents_release_activation() {
        for removed in [false, true] {
            let buttons = buttons();
            let graph = graph(&buttons);
            let mut interaction = ButtonInteraction::default();
            let mut focus = FocusState::default();
            interaction.on_input(
                &pointer(30.0, PointerPhase::Down),
                &buttons,
                &graph,
                &mut focus,
            );
            let mut next = buttons.to_vec();
            if removed {
                next.remove(0);
            } else {
                next[0] = next[0].enabled(false);
            }
            assert_eq!(
                interaction.on_input(&pointer(30.0, PointerPhase::Up), &next, &graph, &mut focus),
                NavigationAction::None
            );
            assert_eq!(focus.current(), None);
            assert_eq!(interaction.pressed(), None);
        }
    }

    #[test]
    fn tab_and_arrows_skip_disabled_actions_and_keyboard_activation_requires_enabled_focus() {
        let mut buttons = buttons();
        let graph = graph(&buttons);
        buttons[1] = buttons[1].enabled(false);
        let mut interaction = ButtonInteraction::default();
        let mut focus = FocusState::new(Some(FIRST), FocusModality::Pointer, true);
        assert_eq!(
            interaction.on_input(&key(Key::Tab, true), &buttons, &graph, &mut focus),
            NavigationAction::FocusChanged(THIRD)
        );
        assert!(focus.is_focus_visible());
        assert_eq!(
            interaction.on_input(&key(Key::ArrowLeft, true), &buttons, &graph, &mut focus),
            NavigationAction::FocusChanged(FIRST)
        );
        assert_eq!(
            interaction.on_input(&key(Key::Enter, true), &buttons, &graph, &mut focus),
            NavigationAction::Activate(FIRST)
        );
        interaction.on_input(&key(Key::Enter, false), &buttons, &graph, &mut focus);
        focus.set_current(Some(SECOND));
        assert_eq!(
            interaction.on_input(&key(Key::Enter, true), &buttons, &graph, &mut focus),
            NavigationAction::None
        );
        assert_eq!(focus.current(), None);
    }

    #[test]
    fn enter_and_space_activate_once_until_release_and_ignore_unfocused_input() {
        let buttons = buttons();
        let graph = graph(&buttons);
        for activation_key in [Key::Enter, Key::Space] {
            let mut interaction = ButtonInteraction::default();
            let mut focus = FocusState::new(Some(FIRST), FocusModality::Keyboard, true);
            assert_eq!(
                interaction.on_input(&key(activation_key, true), &buttons, &graph, &mut focus),
                NavigationAction::Activate(FIRST)
            );
            assert_eq!(interaction.pressed(), Some(FIRST));
            for _ in 0..3 {
                assert_eq!(
                    interaction.on_input(&key(activation_key, true), &buttons, &graph, &mut focus),
                    NavigationAction::None
                );
            }
            interaction.on_input(&InputEvent::Focus(false), &buttons, &graph, &mut focus);
            assert_eq!(interaction.pressed(), None);
            assert_eq!(
                interaction.on_input(&key(Key::Tab, true), &buttons, &graph, &mut focus),
                NavigationAction::None
            );
            assert_eq!(focus.current(), Some(FIRST));
            interaction.on_input(&InputEvent::Focus(true), &buttons, &graph, &mut focus);
            assert_eq!(
                interaction.on_input(&key(activation_key, true), &buttons, &graph, &mut focus),
                NavigationAction::None
            );
            interaction.on_input(&key(activation_key, false), &buttons, &graph, &mut focus);
            assert_eq!(
                interaction.on_input(&key(activation_key, true), &buttons, &graph, &mut focus),
                NavigationAction::Activate(FIRST)
            );
        }
    }

    #[test]
    fn opening_key_can_be_suppressed_until_its_release() {
        let buttons = buttons();
        let graph = graph(&buttons);
        let mut interaction = ButtonInteraction::default();
        let mut focus = FocusState::new(Some(FIRST), FocusModality::Keyboard, true);
        interaction.suppress_activation_until_release(Key::Enter);
        assert_eq!(
            interaction.on_input(&key(Key::Enter, true), &buttons, &graph, &mut focus),
            NavigationAction::None
        );
        interaction.on_input(&key(Key::Enter, false), &buttons, &graph, &mut focus);
        assert_eq!(
            interaction.on_input(&key(Key::Enter, true), &buttons, &graph, &mut focus),
            NavigationAction::Activate(FIRST)
        );
    }

    #[test]
    fn all_schemes_render_semantic_fill_label_and_focus_with_a_surface_gap() {
        for kind in [
            ThemeKind::Light,
            ThemeKind::Dark,
            ThemeKind::HighContrastLight,
            ThemeKind::HighContrastDark,
        ] {
            let theme = Theme::by_kind(kind);
            let focus = FocusState::new(Some(FIRST), FocusModality::Keyboard, true);
            for tone in [ButtonTone::Filled, ButtonTone::Tonal] {
                let button = button(FIRST, 10.0).tone(tone).with_icon("Q");
                let list = draw(button, &theme, &ButtonInteraction::default(), &focus);
                let (fill, content) = match tone {
                    ButtonTone::Filled => (theme.color.primary, theme.color.on_primary),
                    ButtonTone::Tonal => {
                        (theme.color.surface_container_high, theme.color.on_surface)
                    }
                };
                assert!(
                    matches!(&list.commands()[0], RenderCmd::Rect { rect, fill: Some(Paint::Solid(color)), border: None, .. } if *rect == button.rect && *color == fill)
                );
                assert!(list.commands().iter().any(|command| matches!(command, RenderCmd::Text { text, color, .. } if text == "Choose" && *color == content)));
                assert!(
                    matches!(&list.commands()[1], RenderCmd::PushOpacity { opacity, .. } if opacity.get() == 0.12)
                );
                assert!(
                    matches!(&list.commands()[2], RenderCmd::Rect { fill: Some(Paint::Solid(color)), .. } if *color == content)
                );
                assert!(matches!(&list.commands()[3], RenderCmd::PopOpacity { .. }));
                let borders: Vec<_> = list
                    .commands()
                    .iter()
                    .filter_map(|command| match command {
                        RenderCmd::Rect {
                            rect,
                            border: Some(border),
                            fill: None,
                            z: 1,
                            ..
                        } => Some((*rect, *border)),
                        _ => None,
                    })
                    .collect();
                assert_eq!(borders.len(), 2);
                assert_eq!(borders[0].1.width(), 3.0);
                assert_eq!(borders[0].1.color(), theme.focus.ring_color);
                assert_eq!(
                    borders[0].0.origin(),
                    button.rect.origin() - Vec2::splat(3.5)
                );
                assert_eq!(borders[1].1.width(), 2.0);
                assert_eq!(borders[1].1.color(), theme.color.surface);
                assert_eq!(borders[1].0.origin(), button.rect.origin() - Vec2::ONE);
                // Focus strokes sort after every sibling's fill/text, including
                // a connected option inserted after this focused control.
                let mut builder = RenderListBuilder::new(Camera2D::default());
                button
                    .draw(
                        &mut builder,
                        &theme,
                        &ButtonInteraction::default(),
                        &focus,
                        Layer::MODAL,
                    )
                    .unwrap();
                super::tests::button(SECOND, 72.0)
                    .draw(
                        &mut builder,
                        &theme,
                        &ButtonInteraction::default(),
                        &focus,
                        Layer::MODAL,
                    )
                    .unwrap();
                let group = builder.finish().unwrap();
                assert!(matches!(
                    group.commands().last(),
                    Some(RenderCmd::Rect {
                        border: Some(_),
                        z: 1,
                        ..
                    })
                ));
            }
        }
    }

    #[test]
    fn pressed_shape_is_immediate_and_reduced_motion_has_no_clock_dependency() {
        let button = button(FIRST, 10.0).with_icon("Q");
        let buttons = [button];
        let graph = graph(&buttons);
        let mut interaction = ButtonInteraction::default();
        let mut focus = FocusState::default();
        let idle = draw(button, &theme(), &interaction, &focus);
        interaction.on_input(
            &pointer(30.0, PointerPhase::Down),
            &buttons,
            &graph,
            &mut focus,
        );
        let pressed = draw(button, &theme(), &interaction, &focus);
        assert_eq!(button.rect(), buttons[0].rect());
        assert!(
            matches!(&idle.commands()[0], RenderCmd::Rect { radii, .. } if radii.top_left() == 30.0)
        );
        assert!(
            matches!(&pressed.commands()[0], RenderCmd::Rect { radii, .. } if radii.top_left() == 14.0)
        );
        assert!(
            matches!(&pressed.commands()[1], RenderCmd::PushOpacity { opacity, .. } if opacity.get() == 0.12)
        );
        assert!(
            matches!(&pressed.commands()[2], RenderCmd::Rect { fill: Some(Paint::Solid(color)), .. } if *color == theme().color.on_surface)
        );
        assert!(matches!(
            &pressed.commands()[3],
            RenderCmd::PopOpacity { .. }
        ));
        let mut reduced = theme();
        reduced.motion.reduced.duration_scale = Percent::new(0).unwrap();
        reduced.motion.reduced.disable_ambient = true;
        for now in [0, 1, 10_000, u64::MAX] {
            let frame = FrameCtx::new(
                Viewport::new(Vec2::splat(320.0)).unwrap(),
                Dpi::new(1.0).unwrap(),
                now,
                reduced,
            );
            assert_eq!(draw(button, &frame.theme(), &interaction, &focus), pressed);
        }
    }

    #[test]
    fn disabled_visuals_use_authored_alpha_and_never_show_focus_or_state_feedback() {
        let buttons = [button(FIRST, 10.0)];
        let graph = graph(&buttons);
        let mut interaction = ButtonInteraction::default();
        let mut focus = FocusState::new(Some(FIRST), FocusModality::Keyboard, true);
        interaction.on_input(&key(Key::Enter, true), &buttons, &graph, &mut focus);
        let list = draw(buttons[0].enabled(false), &theme(), &interaction, &focus);
        assert_eq!(list.commands().len(), 6);
        assert!(
            matches!(&list.commands()[0], RenderCmd::PushOpacity { opacity, .. } if opacity.get() == 0.12)
        );
        assert!(
            matches!(&list.commands()[1], RenderCmd::Rect { fill: Some(Paint::Solid(color)), border: None, .. } if *color == theme().color.on_surface)
        );
        assert!(matches!(&list.commands()[2], RenderCmd::PopOpacity { .. }));
        assert!(
            matches!(&list.commands()[3], RenderCmd::PushOpacity { opacity, .. } if opacity.get() == 0.38)
        );
        assert!(
            matches!(&list.commands()[4], RenderCmd::Text { color, .. } if *color == theme().color.on_surface)
        );
        assert!(matches!(&list.commands()[5], RenderCmd::PopOpacity { .. }));
    }
}
