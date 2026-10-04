//! Small native standalone setup built on the existing action primitive.
//! No match or canonical clock exists until the Start activation is accepted.

#![allow(clippy::float_arithmetic)]

use glam::Vec2;
use tabula_game_api::AssetRef;
use tabula_presentation::{
    ActionButton, Align, ButtonInteraction, ButtonTone, Camera2D, Corners, FocusGraph, FocusId,
    FocusNode, FocusState, FrameCtx, InputEvent, Layer, NavigationAction, Paint, Rect, RenderCmd,
    RenderList, RenderListBuilder, RenderListError, TextStyleToken,
};

use crate::clock_options::{
    LocalClockControl, LocalClockOptions, MAX_ADJUSTMENT_MS, MAX_INITIAL_MS,
};

const START: FocusId = FocusId::new(7);

#[derive(Debug)]
pub struct StandaloneSetup {
    clock: LocalClockOptions,
    interaction: ButtonInteraction,
    focus: FocusState,
}

impl StandaloneSetup {
    pub fn new(clock: LocalClockOptions) -> Self {
        let mut focus = FocusState::default();
        focus.set_keyboard_focus(Some(START));
        Self {
            clock,
            interaction: ButtonInteraction::default(),
            focus,
        }
    }

    pub fn suppress_held_key(&mut self, key: tabula_presentation::Key) {
        self.interaction.suppress_activation_until_release(key);
    }

    pub fn on_input(&mut self, event: &InputEvent, frame: &FrameCtx) -> Option<LocalClockOptions> {
        let buttons = setup_buttons(frame, self.clock).ok()?;
        let graph = setup_graph(&buttons);
        let NavigationAction::Activate(id) =
            self.interaction
                .on_input(event, &buttons, &graph, &mut self.focus)
        else {
            return None;
        };
        match id.get() {
            0 => self.clock.control = LocalClockControl::Untimed,
            1 => self.clock.control = LocalClockControl::Fischer,
            2 => self.clock.control = LocalClockControl::Bronstein,
            3 => self.clock.initial_ms = self.clock.initial_ms.saturating_sub(60_000).max(60_000),
            4 => {
                self.clock.initial_ms = self
                    .clock
                    .initial_ms
                    .saturating_add(60_000)
                    .min(MAX_INITIAL_MS);
            }
            5 => self.clock.adjustment_ms = self.clock.adjustment_ms.saturating_sub(1_000),
            6 => {
                self.clock.adjustment_ms = self
                    .clock
                    .adjustment_ms
                    .saturating_add(1_000)
                    .min(MAX_ADJUSTMENT_MS);
            }
            7 => return Some(self.clock),
            _ => {}
        }
        None
    }

    #[allow(clippy::too_many_lines)] // One small render composition shares setup hit geometry.
    pub fn present(
        &self,
        frame: &FrameCtx,
        cover: AssetRef,
    ) -> Result<RenderList, RenderListError> {
        let mut builder = RenderListBuilder::new(Camera2D::default());
        let size = frame.viewport().size();
        let theme = frame.theme();
        let art = theme.game_art.chess; // xtask-allow-game-id: direct Phase 2 local standalone entry material.
        fill(&mut builder, Rect::new(Vec2::ZERO, size)?, art.page, 0.0)?;
        // Only this narrow shared brand bar uses canonical system roles.
        fill(
            &mut builder,
            Rect::new(Vec2::ZERO, Vec2::new(size.x, 56.0))?,
            theme.color.surface_container,
            0.0,
        )?;
        text(
            &mut builder,
            "Tabula",
            Vec2::new(24.0, 12.0),
            TextStyleToken::TitleLg,
            theme.color.on_surface,
            None,
        )?;
        text(
            &mut builder,
            "LOCAL / OFFLINE",
            Vec2::new((size.x - 150.0).max(136.0), 17.0),
            TextStyleToken::LabelSm,
            theme.color.on_surface_variant,
            None,
        )?;
        let layout = SetupLayout::new(size)?;
        let panel = layout.panel;
        fill(&mut builder, panel, art.surface, theme.shape.card.get())?;
        if let Some(rect) = layout.cover {
            builder.push(RenderCmd::Sprite {
                asset: cover,
                rect,
                tint: art.piece_tint,
                rotation: 0.0,
                pivot: Vec2::ZERO,
                layer: Layer::PIECES,
                z: 0,
            })?;
            text(
                &mut builder,
                "CHESS", // xtask-allow-game-id: direct Phase 2 local standalone entry label.
                Vec2::new(rect.origin().x, rect.origin().y + rect.size().y + 14.0),
                TextStyleToken::HeadlineLg,
                art.ink,
                None,
            )?;
            text(
                &mut builder,
                "A shared table. A real game.",
                Vec2::new(rect.origin().x, rect.origin().y + rect.size().y + 64.0),
                TextStyleToken::BodyMd,
                art.muted,
                Some(rect.size().x),
            )?;
        }
        let x = layout.controls.origin().x;
        let y = layout.controls.origin().y;
        let width = layout.controls.size().x;
        text(
            &mut builder,
            "New local game",
            Vec2::new(x, y),
            TextStyleToken::HeadlineMd,
            art.ink,
            Some(width),
        )?;
        if !layout.compact {
            text(
                &mut builder,
                "Two players on this device",
                Vec2::new(x, y + 42.0),
                TextStyleToken::BodyMd,
                art.muted,
                Some(width),
            )?;
        }
        text(
            &mut builder,
            "Time control",
            Vec2::new(x, y + if layout.compact { 44.0 } else { 94.0 }),
            TextStyleToken::LabelLg,
            art.ink,
            None,
        )?;
        let initial = if self.clock.timed() {
            format!(
                "{}:{:02} each",
                self.clock.initial_ms / 60_000,
                (self.clock.initial_ms / 1_000) % 60
            )
        } else {
            String::from("No clock")
        };
        text(
            &mut builder,
            &initial,
            Vec2::new(x + 62.0, y + if layout.compact { 134.0 } else { 192.0 }),
            TextStyleToken::TitleMd,
            art.ink,
            Some(width - 124.0),
        )?;
        let adjustment = match self.clock.control {
            LocalClockControl::Untimed => String::from("Take your time"),
            LocalClockControl::Fischer => {
                format!("+{}s per move", self.clock.adjustment_ms / 1_000)
            }
            LocalClockControl::Bronstein => format!("{}s delay", self.clock.adjustment_ms / 1_000),
        };
        text(
            &mut builder,
            &adjustment,
            Vec2::new(x + 62.0, y + if layout.compact { 188.0 } else { 250.0 }),
            TextStyleToken::BodyMd,
            art.ink,
            Some(width - 124.0),
        )?;
        if !layout.compact {
            text(&mut builder, "White starts. Tap or drag a piece. Arrow keys move focus; Enter selects; Escape cancels.", Vec2::new(x, y + 310.0), TextStyleToken::BodySm, art.muted, Some(width))?;
        }
        let buttons = setup_buttons(frame, self.clock)?;
        for button in buttons {
            button.draw(
                &mut builder,
                &theme,
                &self.interaction,
                &self.focus,
                Layer::HUD,
            )?;
        }
        builder.finish()
    }
}

#[derive(Clone, Copy, Debug)]
struct SetupLayout {
    panel: Rect,
    controls: Rect,
    cover: Option<Rect>,
    compact: bool,
}

impl SetupLayout {
    fn new(size: Vec2) -> Result<Self, RenderListError> {
        let wide = size.x >= 800.0 && size.y >= 600.0;
        let width = (size.x - 32.0).clamp(288.0, if wide { 1_000.0 } else { 520.0 });
        let compact = size.y < 558.0;
        let height = if compact { 322.0 } else { 470.0 };
        let panel = Rect::new(
            Vec2::new(
                (size.x - width) / 2.0,
                if compact {
                    64.0
                } else {
                    72.0 + ((size.y - 558.0) / 2.0).max(0.0)
                },
            ),
            Vec2::new(width, height),
        )?;
        let padding = 20.0;
        let cover = if wide {
            Some(Rect::new(
                panel.origin() + Vec2::splat(padding),
                Vec2::new(width * 0.4, width * 0.4 / 1.5),
            )?)
        } else {
            None
        };
        let controls_x = cover.map_or(panel.origin().x + padding, |rect| {
            rect.origin().x + rect.size().x + 32.0
        });
        let controls = Rect::new(
            Vec2::new(controls_x, panel.origin().y + padding),
            Vec2::new(
                panel.origin().x + width - padding - controls_x,
                height - 2.0 * padding,
            ),
        )?;
        Ok(Self {
            panel,
            controls,
            cover,
            compact,
        })
    }
}

fn setup_buttons(
    frame: &FrameCtx,
    clock: LocalClockOptions,
) -> Result<Vec<ActionButton<'static>>, RenderListError> {
    let theme = frame.theme();
    let layout = SetupLayout::new(frame.viewport().size())?;
    let origin = layout.controls.origin();
    let width = layout.controls.size().x;
    let option_width = (width - 12.0) / 3.0;
    let mut buttons = Vec::with_capacity(8);
    for (index, (label, control)) in [
        ("Untimed", LocalClockControl::Untimed),
        ("Fischer", LocalClockControl::Fischer),
        ("Bronstein", LocalClockControl::Bronstein),
    ]
    .into_iter()
    .enumerate()
    {
        #[allow(clippy::cast_precision_loss)]
        let x = index as f32 * (option_width + 6.0);
        buttons.push(
            ActionButton::new(
                FocusId::new(u32::try_from(index).expect("three options")),
                Rect::new(
                    origin + Vec2::new(x, if layout.compact { 68.0 } else { 124.0 }),
                    Vec2::new(option_width, 48.0),
                )?,
                label,
                theme.density.min_target,
            )?
            .tone(if clock.control == control {
                ButtonTone::Filled
            } else {
                ButtonTone::Tonal
            }),
        );
    }
    let initial_y = if layout.compact { 126.0 } else { 184.0 };
    let adjustment_y = if layout.compact { 180.0 } else { 242.0 };
    for (id, x, y, label) in [
        (3, 0.0, initial_y, "-"),
        (4, width - 48.0, initial_y, "+"),
        (5, 0.0, adjustment_y, "-"),
        (6, width - 48.0, adjustment_y, "+"),
    ] {
        let enabled = clock.timed()
            && match id {
                3 => clock.initial_ms > 60_000,
                4 => clock.initial_ms < MAX_INITIAL_MS,
                5 => clock.adjustment_ms > 0,
                _ => clock.adjustment_ms < MAX_ADJUSTMENT_MS,
            };
        buttons.push(
            ActionButton::new(
                FocusId::new(id),
                Rect::new(origin + Vec2::new(x, y), Vec2::splat(48.0))?,
                label,
                theme.density.min_target,
            )?
            .enabled(enabled),
        );
    }
    buttons.push(
        ActionButton::new(
            START,
            Rect::new(
                origin + Vec2::new(0.0, if layout.compact { 234.0 } else { 376.0 }),
                Vec2::new(width, 48.0),
            )?,
            "Start game",
            theme.density.min_target,
        )?
        .tone(ButtonTone::Filled),
    );
    Ok(buttons)
}

fn setup_graph(buttons: &[ActionButton<'_>]) -> FocusGraph {
    FocusGraph::new(
        buttons
            .iter()
            .map(|button| FocusNode::new(button.id(), button.rect()))
            .collect(),
    )
    .expect("distinct setup actions")
}

fn fill(
    builder: &mut RenderListBuilder,
    rect: Rect,
    color: tabula_design::Color,
    radius: f32,
) -> Result<(), RenderListError> {
    builder.push(RenderCmd::Rect {
        rect,
        radii: Corners::uniform(radius)?,
        fill: Some(Paint::Solid(color)),
        border: None,
        layer: Layer::BOARD,
        z: 0,
    })
}

fn text(
    builder: &mut RenderListBuilder,
    value: &str,
    at: Vec2,
    style: TextStyleToken,
    color: tabula_design::Color,
    width: Option<f32>,
) -> Result<(), RenderListError> {
    builder.push(RenderCmd::Text {
        text: value.to_owned(),
        at,
        style,
        align: Align::Start,
        max_width: width
            .map(tabula_design::Positive::new)
            .transpose()
            .map_err(|_| RenderListError::InvalidGeometry)?,
        color,
        layer: Layer::HUD,
        z: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tabula_design::{Theme, ThemeKind};
    use tabula_presentation::{Dpi, Key, Viewport};

    fn frame() -> FrameCtx {
        FrameCtx::new(
            Viewport::new(Vec2::new(390.0, 844.0)).unwrap(),
            Dpi::new(1.0).unwrap(),
            0,
            Theme::by_kind(ThemeKind::Light),
        )
    }

    #[test]
    fn entry_keyboard_start_is_one_shot_and_repeated_key_is_suppressed() {
        let mut setup = StandaloneSetup::new(LocalClockOptions::default());
        let key = InputEvent::Key {
            key: Key::Enter,
            pressed: true,
        };
        assert_eq!(
            setup.on_input(&key, &frame()),
            Some(LocalClockOptions::default())
        );
        assert_eq!(setup.on_input(&key, &frame()), None);
    }

    #[test]
    fn held_key_from_previous_session_cannot_start_the_next_game() {
        let mut setup = StandaloneSetup::new(LocalClockOptions::default());
        setup.suppress_held_key(Key::Enter);
        assert_eq!(
            setup.on_input(
                &InputEvent::Key {
                    key: Key::Enter,
                    pressed: true
                },
                &frame()
            ),
            None
        );
        setup.on_input(
            &InputEvent::Key {
                key: Key::Enter,
                pressed: false,
            },
            &frame(),
        );
        assert!(setup
            .on_input(
                &InputEvent::Key {
                    key: Key::Enter,
                    pressed: true
                },
                &frame()
            )
            .is_some());
    }

    #[test]
    fn setup_actions_have_nonoverlapping_44dp_targets_inside_supported_viewports() {
        for size in [
            Vec2::new(320.0, 568.0),
            Vec2::new(390.0, 844.0),
            Vec2::new(900.0, 720.0),
            Vec2::new(844.0, 390.0),
            Vec2::new(320.0, 390.0),
        ] {
            let frame = FrameCtx::new(
                Viewport::new(size).unwrap(),
                Dpi::new(1.0).unwrap(),
                0,
                Theme::by_kind(ThemeKind::Light),
            );
            let buttons = setup_buttons(&frame, LocalClockOptions::default()).unwrap();
            for button in &buttons {
                assert!(button.rect().size().min_element() >= 44.0);
                assert!(button.rect().origin().min_element() >= 0.0);
                assert!((button.rect().origin() + button.rect().size())
                    .cmple(size)
                    .all());
            }
        }
    }
}
