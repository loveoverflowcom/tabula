//! Local Chess material, clocks and bounded control HUD (doc 04 §5, I-10/I-12).
//! Every consequential action is supplied by the rules projection and confirmed
//! against the same visible position. Observed moves are session-local coordinates,
//! not SAN, authoritative history, or a replay service.
#![allow(
    clippy::float_arithmetic,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

use super::{
    clock_remaining, color_name, format_clock, outline, piece_name, piece_sprite, result_title,
    square_name, status_text, A11yAction, A11yDescription, ActionButton, ActionId, Align,
    BoardLayout, ButtonInteraction, ButtonShape, ButtonTone, ChessColor, ChessLocal, Command,
    Corners, FocusGraph, FocusId, FocusNode, FrameCtx, InputEvent, Intent, Interaction, Key, Layer,
    NavigationAction, Paint, Piece, PieceKind, Rect, RenderCmd, RenderListBuilder, RenderListError,
    SemanticTint, Square, Status, TextStyleToken, Theme, Vec2, View,
};

const FLIP: FocusId = FocusId::new(200);
const DRAW: FocusId = FocusId::new(201);
const DECLINE: FocusId = FocusId::new(202);
const RESIGN: FocusId = FocusId::new(203);
const CLAIM: FocusId = FocusId::new(204);
const FOLLOW: FocusId = FocusId::new(205);
const CONTROL_TOP: FocusId = FocusId::new(206);
const CONTROL_BOTTOM: FocusId = FocusId::new(207);
const CANCEL: FocusId = FocusId::new(300);
const CONFIRM: FocusId = FocusId::new(301);
const MAX_OBSERVED_MOVES: usize = 256;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct VisiblePosition {
    board: [Option<Piece>; 64],
    turn: ChessColor,
    you: Option<ChessColor>,
    fullmove: u16,
    halfmove: u16,
    draw_offer: Option<ChessColor>,
    status: Status,
    actions: Vec<Command>,
}

impl VisiblePosition {
    pub(super) fn new(view: &View) -> Self {
        Self {
            board: view.board,
            turn: view.turn,
            you: view.you,
            fullmove: view.fullmove_number,
            halfmove: view.halfmove_clock,
            draw_offer: view.draw_offer,
            status: view.status.clone(),
            actions: view.actions.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ControlConfirmation {
    command: Command,
    observed: VisiblePosition,
    return_focus: FocusId,
    buttons: ButtonInteraction,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ObservedMove {
    color: ChessColor,
    from: Square,
    to: Square,
    promotion: Option<PieceKind>,
    captured: bool,
}

pub(super) fn record_move(
    local: &mut ChessLocal,
    color: ChessColor,
    from: Square,
    to: Square,
    promotion: Option<PieceKind>,
    captured: bool,
) {
    if local.move_history.len() == MAX_OBSERVED_MOVES {
        local.move_history.remove(0);
    }
    local.move_history.push(ObservedMove {
        color,
        from,
        to,
        promotion,
        captured,
    });
}

fn player_colors(layout: BoardLayout) -> [ChessColor; 2] {
    if layout.flipped {
        [ChessColor::White, ChessColor::Black]
    } else {
        [ChessColor::Black, ChessColor::White]
    }
}

fn controls(view: &View, local: &ChessLocal, layout: BoardLayout) -> Vec<ActionButton<'static>> {
    let metrics = Theme::by_kind(tabula_design::ThemeKind::Light);
    let mut specs = vec![(FLIP, "Flip", true)];
    if view.actions.contains(&Command::AcceptDraw) {
        specs.push((DRAW, "Accept", true));
        specs.push((DECLINE, "Decline", true));
    } else {
        specs.push((DRAW, "Draw", view.actions.contains(&Command::OfferDraw)));
    }
    if view.actions.contains(&Command::ClaimDraw) {
        specs.push((CLAIM, "Claim", true));
    }
    specs.push((RESIGN, "Resign", view.actions.contains(&Command::Resign)));
    if local.control_color.is_some() {
        specs.push((FOLLOW, "Follow", true));
    }
    let short_players = layout.top_player.size().y < 44.0;
    if short_players && local.hot_seat_controls && matches!(view.status, Status::Playing) {
        let colors = player_colors(layout);
        specs.push((
            CONTROL_TOP,
            if colors[0] == ChessColor::White {
                "White"
            } else {
                "Black"
            },
            true,
        ));
        specs.push((
            CONTROL_BOTTOM,
            if colors[1] == ChessColor::White {
                "White"
            } else {
                "Black"
            },
            true,
        ));
    }
    let gap = 4.0;
    let count = u16::try_from(specs.len()).unwrap_or(8);
    let columns = (((layout.controls.size().x + gap) / 76.0).floor() as u16)
        .max(1)
        .min(count);
    let width = (layout.controls.size().x - gap * f32::from(columns.saturating_sub(1)))
        / f32::from(columns);
    let mut buttons = Vec::new();
    for (index, (id, label, enabled)) in specs.into_iter().enumerate() {
        let index = u16::try_from(index).unwrap_or(0);
        let column = f32::from(index % columns);
        let row = f32::from(index / columns);
        if let Ok(rect) = Rect::new(
            layout.controls.origin() + Vec2::new(column * (width + gap), row * 48.0),
            Vec2::new(width, 44.0),
        ) {
            if rect.origin().y + rect.size().y
                <= layout.controls.origin().y + layout.controls.size().y
            {
                if let Ok(button) = ActionButton::new(id, rect, label, metrics.density.min_target) {
                    buttons.push(button.enabled(enabled).shape(ButtonShape::Square));
                }
            }
        }
    }
    if !short_players && local.hot_seat_controls && matches!(view.status, Status::Playing) {
        for (id, rect, color) in [
            (CONTROL_TOP, layout.top_player, player_colors(layout)[0]),
            (
                CONTROL_BOTTOM,
                layout.bottom_player,
                player_colors(layout)[1],
            ),
        ] {
            if let Ok(button) = ActionButton::new(
                id,
                rect,
                if color == ChessColor::White {
                    "Control White"
                } else {
                    "Control Black"
                },
                metrics.density.min_target,
            ) {
                buttons.push(button);
            }
        }
    }
    buttons
}

fn graph(buttons: &[ActionButton<'_>]) -> FocusGraph {
    let enabled: Vec<_> = buttons
        .iter()
        .filter(|button| button.is_enabled())
        .collect();
    let nodes = enabled
        .iter()
        .enumerate()
        .map(|(index, button)| {
            FocusNode::with_neighbors(
                button.id(),
                button.rect(),
                None,
                None,
                index.checked_sub(1).map(|previous| enabled[previous].id()),
                enabled.get(index + 1).map(|next| next.id()),
            )
        })
        .collect();
    FocusGraph::new(nodes).expect("unique local HUD action ids")
}

fn confirmation_geometry(layout: BoardLayout) -> Option<(Rect, Rect, Rect)> {
    let size = layout.viewport.size();
    let panel_size = Vec2::new((size.x - 24.0).min(440.0), 214.0_f32.min(size.y - 24.0));
    if panel_size.x < 180.0 || panel_size.y < 160.0 {
        return None;
    }
    let panel = Rect::new((size - panel_size) * 0.5, panel_size).ok()?;
    let width = (panel_size.x - 36.0) / 2.0;
    let y = panel.origin().y + panel_size.y - 60.0;
    let cancel = Rect::new(
        Vec2::new(panel.origin().x + 12.0, y),
        Vec2::new(width, 48.0),
    )
    .ok()?;
    let confirm = Rect::new(
        Vec2::new(panel.origin().x + 24.0 + width, y),
        Vec2::new(width, 48.0),
    )
    .ok()?;
    Some((panel, cancel, confirm))
}

fn confirmation_buttons(
    view: &View,
    modal: &ControlConfirmation,
    layout: BoardLayout,
) -> Vec<ActionButton<'static>> {
    let metrics = Theme::by_kind(tabula_design::ThemeKind::Light);
    let Some((_, cancel, confirm)) = confirmation_geometry(layout) else {
        return Vec::new();
    };
    let valid =
        modal.observed == VisiblePosition::new(view) && view.actions.contains(&modal.command);
    let label = match modal.command {
        Command::Resign => "Resign",
        Command::OfferDraw => "Offer draw",
        Command::AcceptDraw => "Accept draw",
        Command::ClaimDraw => "Claim draw",
        _ => "Confirm",
    };
    [
        (CANCEL, cancel, "Keep playing", true),
        (CONFIRM, confirm, label, valid),
    ]
    .into_iter()
    .filter_map(|(id, rect, label, enabled)| {
        ActionButton::new(id, rect, label, metrics.density.min_target)
            .ok()
            .map(|button| {
                button
                    .enabled(enabled)
                    .shape(ButtonShape::Square)
                    .tone(if id == CONFIRM {
                        ButtonTone::Filled
                    } else {
                        ButtonTone::Tonal
                    })
            })
    })
    .collect()
}

/// Distinguishes board routing from a consumed local HUD action.
pub(super) enum HudInput {
    Board,
    Handled(Option<Intent<Command>>),
}

/// A handled value owns this input, even when it emits no command.
#[allow(clippy::too_many_lines)]
pub(super) fn on_input(
    input: &InputEvent,
    view: &View,
    local: &mut ChessLocal,
    layout: BoardLayout,
) -> HudInput {
    if let Some(mut modal) = local.confirmation.take() {
        let buttons = confirmation_buttons(view, &modal, layout);
        let graph = graph(&buttons);
        let action = modal
            .buttons
            .on_input(input, &buttons, &graph, &mut local.focus);
        match action {
            NavigationAction::Activate(CANCEL) | NavigationAction::Cancel => {
                local.hud_buttons = ButtonInteraction::default();
                local.focus.set_current(Some(modal.return_focus));
            }
            NavigationAction::Activate(CONFIRM)
                if modal.observed == VisiblePosition::new(view)
                    && view.actions.contains(&modal.command) =>
            {
                local.hud_buttons = ButtonInteraction::default();
                local.focus.set_current(Some(modal.return_focus));
                if let InputEvent::Key {
                    key: Key::Enter,
                    pressed: true,
                } = input
                {
                    local.activation_keys.enter_held = true;
                }
                if let InputEvent::Key {
                    key: Key::Space,
                    pressed: true,
                } = input
                {
                    local.activation_keys.space_held = true;
                }
                return HudInput::Handled(Some(Intent::new(modal.command)));
            }
            _ => local.confirmation = Some(modal),
        }
        return HudInput::Handled(None);
    }
    if matches!(local.interaction, Interaction::Promotion { .. }) {
        return HudInput::Board;
    }
    if matches!(input, InputEvent::Pointer { .. })
        && matches!(
            local.interaction,
            Interaction::Pressed { .. } | Interaction::Dragging { .. }
        )
    {
        return HudInput::Board;
    }
    let buttons = controls(view, local, layout);
    let graph = graph(&buttons);
    // Completion owns Tab in the shell, so keep a board-to-HUD route on the
    // existing directional keys. It changes local focus only, never ownership.
    if matches!(view.status, Status::Ended { .. }) {
        if matches!(
            input,
            InputEvent::Key {
                key: Key::ArrowDown,
                pressed: true
            }
        ) {
            let bottom_rank = if layout.flipped { 7 } else { 0 };
            let at_bottom = local
                .focus
                .current()
                .and_then(|id| u8::try_from(id.get()).ok())
                .and_then(Square::new)
                .is_some_and(|square| square.rank() == bottom_rank);
            if at_bottom {
                if let Some(id) = graph.first_id() {
                    local.focus.set_keyboard_focus(Some(id));
                    return HudInput::Handled(None);
                }
            }
        }
        if matches!(
            input,
            InputEvent::Key {
                key: Key::ArrowUp,
                pressed: true
            }
        ) && local.focus.current() == graph.first_id()
            && graph.first_id().is_some()
        {
            local.hud_buttons = ButtonInteraction::default();
            local
                .focus
                .set_keyboard_focus(Some(FocusId::new(if layout.flipped { 63 } else { 0 })));
            return HudInput::Handled(None);
        }
    }
    if let InputEvent::Key {
        key: Key::Tab,
        pressed: true,
    } = input
    {
        if local.focus.current() == Some(FocusId::new(63)) {
            if let Some(id) = graph.first_id() {
                local.focus.set_keyboard_focus(Some(id));
                return HudInput::Handled(None);
            }
        } else if local.focus.current() == graph.nodes().last().map(FocusNode::id)
            && graph.nodes().last().map(FocusNode::id).is_some()
        {
            local.hud_buttons = ButtonInteraction::default();
            local.focus.set_keyboard_focus(Some(FocusId::new(0)));
            return HudInput::Handled(None);
        }
    }
    let owns = match input {
        InputEvent::Pointer { position, .. } => {
            buttons
                .iter()
                .any(|button| button.rect().contains(position.get()))
                || local.hud_buttons.pressed().is_some()
        }
        InputEvent::Key { .. } => local.focus.current().is_some_and(|id| graph.contains(id)),
        InputEvent::Focus(_) => local.focus.current().is_some_and(|id| graph.contains(id)),
    };
    if !owns {
        return HudInput::Board;
    }
    let action = local
        .hud_buttons
        .on_input(input, &buttons, &graph, &mut local.focus);
    let NavigationAction::Activate(id) = action else {
        return HudInput::Handled(None);
    };
    if id == FLIP {
        local.flipped = !local.flipped;
        local.clear_interaction();
        local.move_animation = None;
        return HudInput::Handled(None);
    }
    if id == CONTROL_TOP || id == CONTROL_BOTTOM {
        local.control_color = Some(player_colors(layout)[usize::from(id == CONTROL_BOTTOM)]);
        local.clear_interaction();
        return HudInput::Handled(None);
    }
    if id == FOLLOW {
        local.control_color = None;
        local.clear_interaction();
        return HudInput::Handled(None);
    }
    let command = match id {
        RESIGN => Command::Resign,
        DRAW if view.actions.contains(&Command::AcceptDraw) => Command::AcceptDraw,
        DRAW => Command::OfferDraw,
        DECLINE => Command::DeclineDraw,
        CLAIM => Command::ClaimDraw,
        _ => return HudInput::Handled(None),
    };
    if !view.actions.contains(&command) {
        return HudInput::Handled(None);
    }
    local.clear_interaction();
    if command == Command::DeclineDraw {
        return HudInput::Handled(Some(Intent::new(command)));
    }
    let mut modal = ControlConfirmation {
        command,
        observed: VisiblePosition::new(view),
        return_focus: id,
        buttons: ButtonInteraction::default(),
    };
    if let InputEvent::Key { key, pressed: true } = input {
        modal.buttons.suppress_activation_until_release(*key);
    }
    local.confirmation = Some(modal);
    local.focus.set_current(Some(CANCEL));
    HudInput::Handled(None)
}

#[allow(clippy::too_many_arguments)]
fn text(
    builder: &mut RenderListBuilder,
    value: impl Into<String>,
    at: Vec2,
    style: TextStyleToken,
    color: SemanticTint,
    max_width: f32,
    layer: Layer,
    z: i16,
) -> Result<(), RenderListError> {
    if max_width <= 0.0 {
        return Ok(());
    }
    builder.push(RenderCmd::Text {
        text: value.into(),
        at,
        style,
        align: Align::Start,
        max_width: Some(
            tabula_design::Positive::new(max_width)
                .map_err(|_| RenderListError::InvalidGeometry)?,
        ),
        color,
        layer,
        z,
    })
}

pub(super) fn draw_material(
    builder: &mut RenderListBuilder,
    frame: &FrameCtx,
    layout: BoardLayout,
) -> Result<(), RenderListError> {
    let theme = frame.theme();
    let art = theme.game_art.chess;
    for (rect, color, radius) in [
        (
            Rect::new(Vec2::ZERO, layout.viewport.size())?,
            art.page,
            0.0,
        ),
        (layout.table, art.deep, theme.shape.card.get()),
    ] {
        builder.push(RenderCmd::Rect {
            rect,
            radii: Corners::uniform(radius)?,
            fill: Some(Paint::Solid(color)),
            border: None,
            layer: Layer::BOARD,
            z: -10,
        })?;
    }
    if layout.title.size().y >= 24.0 {
        text(
            builder,
            "Chess",
            layout.title.origin(),
            if layout.title.size().y >= 36.0 {
                TextStyleToken::DisplaySm
            } else {
                TextStyleToken::TitleLg
            },
            art.ink,
            layout.title.size().x,
            Layer::HUD,
            0,
        )?;
    }
    if layout.square_size() >= 12.0 {
        for value in 0..8_u8 {
            let file = if layout.flipped { 7 - value } else { value };
            let rank = if layout.flipped { value + 1 } else { 8 - value };
            let x = layout.board.origin().x
                + f32::from(value) * layout.square_size()
                + layout.square_size() * 0.4;
            text(
                builder,
                char::from(b'a' + file).to_string(),
                Vec2::new(x, layout.board.origin().y + layout.board.size().y + 1.0),
                TextStyleToken::LabelSm,
                art.on_deep,
                layout.square_size(),
                Layer::HUD,
                0,
            )?;
            text(
                builder,
                rank.to_string(),
                Vec2::new(
                    layout.table.origin().x + 2.0,
                    layout.board.origin().y
                        + f32::from(value) * layout.square_size()
                        + layout.square_size() * 0.35,
                ),
                TextStyleToken::LabelSm,
                art.on_deep,
                18.0,
                Layer::HUD,
                0,
            )?;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
pub(super) fn draw(
    builder: &mut RenderListBuilder,
    view: &View,
    local: &ChessLocal,
    frame: &FrameCtx,
    layout: BoardLayout,
) -> Result<(), RenderListError> {
    let theme = frame.theme();
    let art = theme.game_art.chess;
    let remaining = clock_remaining(view, frame);
    for (index, (color, rect)) in player_colors(layout)
        .into_iter()
        .zip([layout.top_player, layout.bottom_player])
        .enumerate()
    {
        if rect.size().y < 24.0 || rect.size().x < 48.0 {
            continue;
        }
        let active = matches!(view.status, Status::Playing) && view.turn == color;
        let fill = if active { art.surface } else { art.soft };
        builder.push(RenderCmd::Rect {
            rect,
            radii: Corners::uniform(theme.shape.button.get().min(rect.size().y * 0.4))?,
            fill: Some(Paint::Solid(fill)),
            border: None,
            layer: Layer::HUD,
            z: 0,
        })?;
        let icon_size = (rect.size().y - 12.0).min(40.0);
        builder.push(piece_sprite(
            Piece {
                color,
                kind: PieceKind::King,
            },
            Rect::new(rect.origin() + Vec2::new(6.0, 6.0), Vec2::splat(icon_size))?,
            &theme,
            Layer::HUD,
            1,
        )?)?;
        let label_x = icon_size + 16.0;
        text(
            builder,
            format!(
                "{}{}",
                color_name(color),
                if active { " / turn" } else { "" }
            ),
            rect.origin() + Vec2::new(label_x, 4.0),
            TextStyleToken::LabelSm,
            art.ink,
            (rect.size().x - label_x - 88.0).max(60.0),
            Layer::HUD,
            1,
        )?;
        if rect.size().y >= 44.0 {
            let label = if local.control_color == Some(color) {
                "Controlling"
            } else if local.hot_seat_controls {
                "Tap to control"
            } else {
                "Local player"
            };
            text(
                builder,
                label,
                rect.origin() + Vec2::new(label_x, 25.0),
                TextStyleToken::LabelSm,
                art.muted,
                (rect.size().x - label_x - 88.0).max(60.0),
                Layer::HUD,
                1,
            )?;
        }
        if let Some(remaining) = remaining {
            let color_index = usize::from(color == ChessColor::Black);
            let low = active && remaining[color_index] <= 30_000;
            let width = 88.0_f32.min(rect.size().x * 0.38);
            let clock_rect = Rect::new(
                rect.origin() + Vec2::new(rect.size().x - width - 6.0, 4.0),
                Vec2::new(width, rect.size().y - 8.0),
            )?;
            builder.push(RenderCmd::Rect {
                rect: clock_rect,
                radii: Corners::uniform(theme.shape.button.get())?,
                fill: Some(Paint::Solid(if low {
                    theme.color.danger
                } else {
                    art.deep
                })),
                border: None,
                layer: Layer::HUD,
                z: 1,
            })?;
            text(
                builder,
                format!(
                    "{}{}",
                    format_clock("", remaining[color_index]).trim(),
                    if low { " LOW" } else { "" }
                ),
                clock_rect.origin() + Vec2::new(6.0, (clock_rect.size().y - 24.0) * 0.5),
                TextStyleToken::MonoMd,
                if low {
                    theme.color.on_danger
                } else {
                    art.on_deep
                },
                width - 12.0,
                Layer::HUD,
                2,
            )?;
        }
        let focus_id = if index == 0 {
            CONTROL_TOP
        } else {
            CONTROL_BOTTOM
        };
        if local.focus.is_focus_visible() && local.focus.current() == Some(focus_id) {
            builder.push(outline(
                rect,
                theme.focus.ring_color,
                Layer::HUD,
                5,
                &theme,
            )?)?;
        }
    }
    let status = layout.status;
    if status.size().x >= 120.0 && status.size().y >= 24.0 {
        builder.push(RenderCmd::Rect {
            rect: status,
            radii: Corners::uniform(theme.shape.card.get())?,
            fill: Some(Paint::Solid(art.surface)),
            border: None,
            layer: Layer::HUD,
            z: 0,
        })?;
        let rail_toolbar = layout.controls.origin().x >= status.origin().x
            && layout.controls.origin().y >= status.origin().y;
        let padding = if rail_toolbar { 6.0 } else { 12.0 };
        let rail = status.size().y > 160.0 && !rail_toolbar;
        let terminal_title = if rail { result_title(view) } else { None };
        text(
            builder,
            terminal_title.clone().unwrap_or_else(|| status_text(view)),
            status.origin() + Vec2::splat(padding),
            if terminal_title.is_some() {
                TextStyleToken::DisplaySm
            } else if rail_toolbar {
                TextStyleToken::LabelSm
            } else {
                TextStyleToken::TitleSm
            },
            if view.in_check {
                theme.color.danger
            } else {
                art.ink
            },
            status.size().x - padding * 2.0,
            Layer::HUD,
            1,
        )?;
        let detail = if let Status::Ended { outcome } = &view.status {
            outcome.summary().to_owned()
        } else if let Some(offer) = view.draw_offer {
            format!("{} offered a draw", color_name(offer))
        } else if view.in_check {
            format!("CHECK / {} king is threatened", color_name(view.turn))
        } else if local.control_color.is_some_and(|color| color != view.turn) {
            "Other seat controlled / Follow to move".into()
        } else if !view.actions.contains(&Command::OfferDraw)
            && matches!(view.status, Status::Playing)
        {
            "Draw offer: choose the last mover".into()
        } else {
            "Select a piece, then a legal destination".into()
        };
        if status.size().y >= 64.0 && !rail_toolbar {
            text(
                builder,
                detail,
                status.origin()
                    + Vec2::new(padding, if terminal_title.is_some() { 64.0 } else { 40.0 }),
                TextStyleToken::BodySm,
                art.muted,
                status.size().x - padding * 2.0,
                Layer::HUD,
                1,
            )?;
        }
        if rail {
            text(
                builder,
                "Observed moves",
                status.origin() + Vec2::new(padding, 96.0),
                TextStyleToken::TitleMd,
                art.ink,
                status.size().x - padding * 2.0,
                Layer::HUD,
                1,
            )?;
            text(
                builder,
                "Coordinates / this session only",
                status.origin() + Vec2::new(padding, 124.0),
                TextStyleToken::LabelSm,
                art.muted,
                status.size().x - padding * 2.0,
                Layer::HUD,
                1,
            )?;
            let room = ((status.size().y - 160.0) / 30.0).floor();
            let visible = local.move_history.len().min(room.max(0.0) as usize);
            let first = local.move_history.len().saturating_sub(visible);
            for (row, movement) in local.move_history.iter().skip(first).enumerate() {
                let row = u16::try_from(row).unwrap_or(0);
                let suffix = movement
                    .promotion
                    .map_or(String::new(), |piece| format!(" = {}", piece_name(piece)));
                text(
                    builder,
                    format!(
                        "{}  {} {} {}{}",
                        color_name(movement.color),
                        square_name(movement.from),
                        if movement.captured { "x" } else { "->" },
                        square_name(movement.to),
                        suffix
                    ),
                    status.origin() + Vec2::new(padding, 156.0 + f32::from(row) * 30.0),
                    TextStyleToken::BodyMd,
                    art.ink,
                    status.size().x - padding * 2.0,
                    Layer::HUD,
                    1,
                )?;
            }
        }
    }
    for button in controls(view, local, layout).into_iter().filter(|button| {
        button.id().get() < CONTROL_TOP.get()
            || (button.rect() != layout.top_player && button.rect() != layout.bottom_player)
    }) {
        button.draw(
            builder,
            &theme,
            &local.hud_buttons,
            &local.focus,
            Layer::HUD,
        )?;
    }
    if let Some(modal) = &local.confirmation {
        if let Some((panel, _, _)) = confirmation_geometry(layout) {
            builder.push(RenderCmd::PushOpacity {
                opacity: tabula_presentation::Opacity::try_from(0.38)
                    .map_err(|_| RenderListError::InvalidGeometry)?,
                layer: Layer::MODAL,
                z: -1,
            })?;
            builder.push(RenderCmd::Rect {
                rect: Rect::new(Vec2::ZERO, layout.viewport.size())?,
                radii: Corners::uniform(0.0)?,
                fill: Some(Paint::Solid(theme.color.hidden)),
                border: None,
                layer: Layer::MODAL,
                z: -1,
            })?;
            builder.push(RenderCmd::PopOpacity {
                layer: Layer::MODAL,
                z: -1,
            })?;
            builder.push(RenderCmd::Rect {
                rect: panel,
                radii: Corners::uniform(theme.shape.sheet.get())?,
                fill: Some(Paint::Solid(art.surface)),
                border: None,
                layer: Layer::MODAL,
                z: 0,
            })?;
            let title = match modal.command {
                Command::Resign => "Resign this game?",
                Command::OfferDraw => "Offer a draw?",
                Command::AcceptDraw => "Accept the draw?",
                Command::ClaimDraw => "Claim this draw?",
                _ => "Confirm action?",
            };
            text(
                builder,
                title,
                panel.origin() + Vec2::splat(16.0),
                TextStyleToken::TitleMd,
                art.ink,
                panel.size().x - 32.0,
                Layer::MODAL,
                1,
            )?;
            let detail = if modal.observed != VisiblePosition::new(view) {
                "Position changed. Return to the board.".into()
            } else if modal.command == Command::Resign {
                format!(
                    "{} resigns. The rules determine the result.",
                    color_name(view.you.unwrap_or(view.turn))
                )
            } else if modal.command == Command::OfferDraw {
                "An offer does not end play. The other seat can accept or decline.".into()
            } else {
                "Confirming ends this game as a draw.".into()
            };
            text(
                builder,
                detail,
                panel.origin() + Vec2::new(16.0, 56.0),
                TextStyleToken::BodyMd,
                art.muted,
                panel.size().x - 32.0,
                Layer::MODAL,
                1,
            )?;
            for button in confirmation_buttons(view, modal, layout) {
                button.draw(builder, &theme, &modal.buttons, &local.focus, Layer::MODAL)?;
            }
        }
    }
    Ok(())
}

/// Describes the same modal eligibility that controls normalized input.
pub(super) fn describe_confirmation(
    description: &mut A11yDescription,
    view: &View,
    local: &ChessLocal,
) {
    if local.hot_seat_controls && !matches!(local.interaction, Interaction::Promotion { .. }) {
        description.actions.extend([
            A11yAction {
                id: ActionId("control-white".into()),
                label: "Control White in this local hot-seat game".into(),
                enabled: local.confirmation.is_none() && matches!(view.status, Status::Playing),
            },
            A11yAction {
                id: ActionId("control-black".into()),
                label: "Control Black in this local hot-seat game".into(),
                enabled: local.confirmation.is_none() && matches!(view.status, Status::Playing),
            },
            A11yAction {
                id: ActionId("follow-turn".into()),
                label: "Follow the current turn".into(),
                enabled: local.confirmation.is_none() && local.control_color.is_some(),
            },
        ]);
    }
    let Some(modal) = &local.confirmation else {
        return;
    };
    let valid =
        modal.observed == VisiblePosition::new(view) && view.actions.contains(&modal.command);
    description.status.push_str(if valid {
        " / confirm game action, Keep playing selected first"
    } else {
        " / position changed, action unavailable"
    });
    description.actions.extend([
        A11yAction {
            id: ActionId("cancel-game-action".into()),
            label: "Keep playing and return to the board".into(),
            enabled: true,
        },
        A11yAction {
            id: ActionId("confirm-game-action".into()),
            label: "Confirm the selected game action".into(),
            enabled: valid,
        },
    ]);
}

#[cfg(test)]
mod tests {
    use super::super::{
        ChessPresentation, Interaction, PointerButton, PointerPhase, PointerPosition, Viewport,
    };
    use super::*;
    use crate::ChessRules;
    use tabula_core::{
        canonical_encode, DetRng, InputIndex, LogicalTime, MatchSeed, SeatId, Viewer,
    };
    use tabula_game_api::{Budget, Ctx, GameRules, Input};
    use tabula_presentation::GamePresentation;

    fn viewport() -> Viewport {
        Viewport::new(Vec2::new(390.0, 844.0)).unwrap()
    }
    fn projected(state: &crate::State, color: ChessColor) -> View {
        ChessRules::project(state, Viewer::Seat(color.seat()))
    }
    fn new_local() -> ChessLocal {
        let mut local = ChessLocal::default();
        local.set_viewport(viewport());
        local
    }
    fn pointer(rect: Rect, phase: PointerPhase) -> InputEvent {
        InputEvent::Pointer {
            position: PointerPosition::new(rect.origin() + rect.size() * 0.5).unwrap(),
            button: PointerButton::Primary,
            phase,
        }
    }
    fn click(view: &View, local: &mut ChessLocal, rect: Rect) -> Option<Intent<Command>> {
        assert!(
            ChessPresentation::on_input(&pointer(rect, PointerPhase::Down), view, local).is_none()
        );
        ChessPresentation::on_input(&pointer(rect, PointerPhase::Up), view, local)
    }
    fn button(view: &View, local: &ChessLocal, id: FocusId) -> Rect {
        controls(
            view,
            local,
            BoardLayout::oriented(viewport(), local.flipped),
        )
        .into_iter()
        .find(|button| button.id() == id)
        .unwrap()
        .rect()
    }
    fn apply(state: &mut crate::State, color: ChessColor, command: Command) {
        let mut rng = DetRng::for_input(&MatchSeed::from_bytes([0; 32]), InputIndex(0));
        let mut ctx = Ctx {
            now: LogicalTime::ZERO,
            index: InputIndex(0),
            rng: &mut rng,
            budget: Budget::default(),
        };
        ChessRules::apply(
            state,
            Input::Player {
                seat: color.seat(),
                command,
            },
            &mut ctx,
        )
        .unwrap();
    }
    #[test]
    fn local_flip_maps_all_square_centers_and_keeps_command_identity() {
        let state = crate::State::initial();
        let view = projected(&state, ChessColor::White);
        let mut local = new_local();
        let before = canonical_encode(&state).unwrap();
        let rect = button(&view, &local, FLIP);
        assert!(click(&view, &mut local, rect).is_none());
        assert!(local.is_flipped());
        assert!(local.viewer_override().is_none());
        let layout = BoardLayout::oriented(viewport(), true);
        for value in 0..64 {
            let square = Square::new(value).unwrap();
            let rect = layout.square_rect(square).unwrap();
            assert_eq!(
                layout.square_at(PointerPosition::new(rect.origin() + rect.size() * 0.5).unwrap()),
                Some(square)
            );
        }
        assert!(click(&view, &mut local, layout.square_rect(Square(12)).unwrap()).is_none());
        assert_eq!(
            click(&view, &mut local, layout.square_rect(Square(28)).unwrap())
                .unwrap()
                .into_command(),
            Command::Move {
                from: 12,
                to: 28,
                promotion: None
            }
        );
        assert_eq!(canonical_encode(&state).unwrap(), before);
    }
    #[test]
    fn hot_seat_override_requires_explicit_shell_admission_and_resets_on_move() {
        let state = crate::State::initial();
        let view = projected(&state, ChessColor::White);
        let mut local = new_local();
        assert!(local.viewer_override().is_none());
        assert!(
            !controls(&view, &local, BoardLayout::from_viewport(viewport()))
                .iter()
                .any(|button| button.id() == CONTROL_TOP)
        );
        local.set_hot_seat_controls(true);
        let rect = button(&view, &local, CONTROL_TOP);
        assert!(click(&view, &mut local, rect).is_none());
        assert_eq!(local.viewer_override(), Some(Viewer::Seat(SeatId(1))));
        local.set_hot_seat_controls(false);
        assert!(local.viewer_override().is_none());
        local.set_hot_seat_controls(true);
        local.control_color = Some(ChessColor::Black);
        let frame = FrameCtx::new(
            viewport(),
            tabula_presentation::Dpi::new(1.0).unwrap(),
            0,
            Theme::by_kind(tabula_design::ThemeKind::Light),
        );
        ChessPresentation::on_view_event(
            &crate::ViewEvent::Moved {
                seat: SeatId(0),
                from: Square(12),
                to: Square(28),
                promotion: None,
                captured: None,
            },
            &mut local,
            &frame,
        );
        assert!(local.viewer_override().is_none());
    }
    #[test]
    fn resign_is_safe_by_default_and_confirmed_once() {
        let state = crate::State::initial();
        let view = projected(&state, ChessColor::White);
        let before = canonical_encode(&state).unwrap();
        let mut local = new_local();
        let rect = button(&view, &local, RESIGN);
        assert!(click(&view, &mut local, rect).is_none());
        assert!(local.confirmation.is_some());
        assert_eq!(local.focus.current(), Some(CANCEL));
        let (_, cancel, confirm) =
            confirmation_geometry(BoardLayout::from_viewport(viewport())).unwrap();
        assert!(click(&view, &mut local, cancel).is_none());
        assert!(local.confirmation.is_none());
        assert!(click(&view, &mut local, rect).is_none());
        assert_eq!(
            click(&view, &mut local, confirm).unwrap().into_command(),
            Command::Resign
        );
        assert!(local.confirmation.is_none());
        assert!(ChessPresentation::on_input(
            &pointer(confirm, PointerPhase::Up),
            &view,
            &mut local
        )
        .is_none());
        assert_eq!(canonical_encode(&state).unwrap(), before);
    }
    #[test]
    fn stale_or_changed_seat_confirmation_cannot_emit_an_action() {
        let state = crate::State::initial();
        let mut view = projected(&state, ChessColor::White);
        let mut local = new_local();
        let rect = button(&view, &local, RESIGN);
        assert!(click(&view, &mut local, rect).is_none());
        view.fullmove_number += 1;
        let (_, cancel, confirm) =
            confirmation_geometry(BoardLayout::from_viewport(viewport())).unwrap();
        assert!(click(&view, &mut local, confirm).is_none());
        let description = ChessPresentation::a11y(&view, &local);
        assert!(description
            .actions
            .iter()
            .any(|action| action.id.0 == "confirm-game-action" && !action.enabled));
        assert!(click(&view, &mut local, cancel).is_none());
        assert!(local.confirmation.is_none());
    }
    #[test]
    fn cancelling_keyboard_confirmation_resets_the_hud_activation_latch() {
        let state = crate::State::initial();
        let view = projected(&state, ChessColor::White);
        let mut local = new_local();
        local.focus.set_keyboard_focus(Some(RESIGN));
        ChessPresentation::on_input(
            &InputEvent::Key {
                key: Key::Enter,
                pressed: true,
            },
            &view,
            &mut local,
        );
        assert!(local.confirmation.is_some());
        ChessPresentation::on_input(
            &InputEvent::Key {
                key: Key::Enter,
                pressed: false,
            },
            &view,
            &mut local,
        );
        ChessPresentation::on_input(
            &InputEvent::Key {
                key: Key::Escape,
                pressed: true,
            },
            &view,
            &mut local,
        );
        assert!(local.confirmation.is_none());
        ChessPresentation::on_input(
            &InputEvent::Key {
                key: Key::Enter,
                pressed: true,
            },
            &view,
            &mut local,
        );
        assert!(local.confirmation.is_some());
    }
    #[test]
    fn blur_during_keyboard_confirmation_does_not_latch_the_opening_action() {
        let state = crate::State::initial();
        let view = projected(&state, ChessColor::White);
        let mut local = new_local();
        local.focus.set_keyboard_focus(Some(RESIGN));
        assert!(ChessPresentation::on_input(
            &InputEvent::Key {
                key: Key::Enter,
                pressed: true
            },
            &view,
            &mut local
        )
        .is_none());
        assert!(local.confirmation.is_some());
        ChessPresentation::on_input(&InputEvent::Focus(false), &view, &mut local);
        ChessPresentation::on_input(&InputEvent::Focus(true), &view, &mut local);
        ChessPresentation::on_input(
            &InputEvent::Key {
                key: Key::Escape,
                pressed: true,
            },
            &view,
            &mut local,
        );
        assert!(local.confirmation.is_none());
        assert_eq!(local.focus.current(), Some(RESIGN));
        assert!(ChessPresentation::on_input(
            &InputEvent::Key {
                key: Key::Enter,
                pressed: true
            },
            &view,
            &mut local
        )
        .is_none());
        assert!(
            local.confirmation.is_some(),
            "first fresh activation after window focus returns must work"
        );
    }
    #[test]
    fn offers_use_projected_offturn_eligibility_and_confirmation() {
        let mut state = crate::State::initial();
        for (color, from, to) in [
            (ChessColor::White, 12, 28),
            (ChessColor::Black, 52, 36),
            (ChessColor::White, 6, 21),
        ] {
            apply(
                &mut state,
                color,
                Command::Move {
                    from,
                    to,
                    promotion: None,
                },
            );
        }
        let view = projected(&state, ChessColor::White);
        assert!(view.legal_moves.is_empty());
        assert!(view.actions.contains(&Command::OfferDraw));
        let mut local = new_local();
        let rect = button(&view, &local, DRAW);
        assert!(click(&view, &mut local, rect).is_none());
        let (_, _, confirm) =
            confirmation_geometry(BoardLayout::from_viewport(viewport())).unwrap();
        let command = click(&view, &mut local, confirm).unwrap().into_command();
        assert_eq!(command, Command::OfferDraw);
        apply(&mut state, ChessColor::White, command);
        let recipient = projected(&state, ChessColor::Black);
        assert!(recipient.actions.contains(&Command::AcceptDraw));
        let mut local = new_local();
        let decline = button(&recipient, &local, DECLINE);
        assert_eq!(
            click(&recipient, &mut local, decline)
                .unwrap()
                .into_command(),
            Command::DeclineDraw
        );
    }
    #[test]
    fn terminal_board_has_directional_keyboard_access_to_flip_in_both_orientations() {
        let mut state = crate::State::initial();
        apply(&mut state, ChessColor::White, Command::Resign);
        let view = projected(&state, ChessColor::White);
        let before = canonical_encode(&state).unwrap();
        for flipped in [false, true] {
            let mut local = new_local();
            local.flipped = flipped;
            local
                .focus
                .set_keyboard_focus(Some(FocusId::new(if flipped { 59 } else { 3 })));
            assert!(ChessPresentation::on_input(
                &InputEvent::Key {
                    key: Key::ArrowDown,
                    pressed: true
                },
                &view,
                &mut local
            )
            .is_none());
            assert_eq!(local.focus.current(), Some(FLIP));
            assert!(ChessPresentation::on_input(
                &InputEvent::Key {
                    key: Key::Enter,
                    pressed: true
                },
                &view,
                &mut local
            )
            .is_none());
            assert_eq!(local.flipped, !flipped);
            assert!(ChessPresentation::on_input(
                &InputEvent::Key {
                    key: Key::Enter,
                    pressed: true
                },
                &view,
                &mut local
            )
            .is_none());
            assert_eq!(
                local.flipped, !flipped,
                "a repeated activation cannot flip again"
            );
            assert!(ChessPresentation::on_input(
                &InputEvent::Key {
                    key: Key::ArrowUp,
                    pressed: true
                },
                &view,
                &mut local
            )
            .is_none());
            assert_eq!(
                local.focus.current(),
                Some(FocusId::new(if flipped { 0 } else { 63 }))
            );
            // The held-key release occurs back on the board; returning to the HUD
            // must still accept the next fresh physical activation.
            ChessPresentation::on_input(
                &InputEvent::Key {
                    key: Key::Enter,
                    pressed: false,
                },
                &view,
                &mut local,
            );
            ChessPresentation::on_input(
                &InputEvent::Key {
                    key: Key::ArrowDown,
                    pressed: true,
                },
                &view,
                &mut local,
            );
            ChessPresentation::on_input(
                &InputEvent::Key {
                    key: Key::Enter,
                    pressed: true,
                },
                &view,
                &mut local,
            );
            assert_eq!(local.flipped, flipped);
        }
        assert_eq!(canonical_encode(&state).unwrap(), before);
    }
    #[test]
    fn opening_draw_control_is_disabled_without_inventing_eligibility() {
        let state = crate::State::initial();
        let view = projected(&state, ChessColor::White);
        let mut local = new_local();
        let button = controls(&view, &local, BoardLayout::from_viewport(viewport()))
            .into_iter()
            .find(|button| button.id() == DRAW)
            .unwrap();
        assert!(!button.is_enabled());
        assert!(click(&view, &mut local, button.rect()).is_none());
        assert!(local.confirmation.is_none());
    }
    #[test]
    fn focus_loss_interrupts_drag_and_blocks_orphan_release_after_resume() {
        let state = crate::State::initial();
        let view = projected(&state, ChessColor::White);
        let mut local = new_local();
        let layout = BoardLayout::from_viewport(viewport());
        let from = layout.square_rect(Square(12)).unwrap();
        let to = layout.square_rect(Square(28)).unwrap();
        ChessPresentation::on_input(&pointer(from, PointerPhase::Down), &view, &mut local);
        ChessPresentation::on_input(&pointer(to, PointerPhase::Move), &view, &mut local);
        assert!(matches!(local.interaction, Interaction::Dragging { .. }));
        ChessPresentation::on_input(&InputEvent::Focus(false), &view, &mut local);
        assert!(matches!(
            local.interaction,
            Interaction::Selected { square: Square(12) }
        ));
        ChessPresentation::on_input(&InputEvent::Focus(true), &view, &mut local);
        assert!(
            ChessPresentation::on_input(&pointer(to, PointerPhase::Up), &view, &mut local)
                .is_none()
        );
        assert!(click(&view, &mut local, to).is_some());
    }
}

#[cfg(test)]
mod responsive_regressions {
    use super::super::{ChessPresentation, PointerButton, PointerPhase, PointerPosition, Viewport};
    use super::*;
    use crate::ChessRules;
    use tabula_core::{SeatId, Viewer};
    use tabula_game_api::GameRules;
    use tabula_presentation::GamePresentation;

    fn input(rect: Rect, phase: PointerPhase) -> InputEvent {
        InputEvent::Pointer {
            position: PointerPosition::new(rect.origin() + rect.size() * 0.5).unwrap(),
            button: PointerButton::Primary,
            phase,
        }
    }
    #[test]
    fn every_eligible_control_retains_44_dp_hit_and_keyboard_targets_in_small_layouts() {
        let state = crate::State::initial();
        let mut view = ChessRules::project(&state, Viewer::Seat(SeatId(0)));
        // Maximum projected control combination, including pending offer and claim.
        view.actions = vec![
            Command::Resign,
            Command::AcceptDraw,
            Command::DeclineDraw,
            Command::ClaimDraw,
        ];
        for (width, height) in [
            (320.0, 568.0),
            (390.0, 844.0),
            (760.0, 360.0),
            (844.0, 390.0),
        ] {
            let viewport = Viewport::new(Vec2::new(width, height)).unwrap();
            let layout = BoardLayout::from_viewport(viewport);
            let mut local = ChessLocal::default();
            local.set_viewport(viewport);
            local.set_hot_seat_controls(true);
            local.control_color = Some(ChessColor::White);
            let buttons = controls(&view, &local, layout);
            let focus_graph = graph(&buttons);
            for id in [
                FLIP,
                DRAW,
                DECLINE,
                RESIGN,
                CLAIM,
                FOLLOW,
                CONTROL_TOP,
                CONTROL_BOTTOM,
            ] {
                let button = buttons
                    .iter()
                    .find(|button| button.id() == id)
                    .unwrap_or_else(|| panic!("missing {id} at {width}x{height}"));
                assert!(button.is_enabled());
                assert!(focus_graph.contains(id));
                assert!(button.rect().size().cmpge(Vec2::splat(44.0)).all());
                assert!(button.rect().origin().cmpge(Vec2::ZERO).all());
                assert!((button.rect().origin() + button.rect().size())
                    .cmple(Vec2::new(width, height))
                    .all());
            }
            local.focus.set_keyboard_focus(Some(FocusId::new(63)));
            ChessPresentation::on_input(
                &InputEvent::Key {
                    key: Key::Tab,
                    pressed: true,
                },
                &view,
                &mut local,
            );
            assert_eq!(local.focus.current(), Some(FLIP));
        }
    }
    #[test]
    fn a_board_drag_keeps_pointer_ownership_across_every_hud_region() {
        let state = crate::State::initial();
        let view = ChessRules::project(&state, Viewer::Seat(SeatId(0)));
        let viewport = Viewport::new(Vec2::new(390.0, 844.0)).unwrap();
        let layout = BoardLayout::from_viewport(viewport);
        let mut admitted = ChessLocal::default();
        admitted.set_viewport(viewport);
        admitted.set_hot_seat_controls(true);
        let mut targets: Vec<_> = controls(&view, &admitted, layout)
            .into_iter()
            .map(ActionButton::rect)
            .collect();
        targets.push(layout.status);
        for target in targets {
            let mut local = admitted.clone();
            let from = layout.square_rect(Square(12)).unwrap();
            let to = layout.square_rect(Square(28)).unwrap();
            ChessPresentation::on_input(&input(from, PointerPhase::Down), &view, &mut local);
            ChessPresentation::on_input(&input(to, PointerPhase::Move), &view, &mut local);
            assert!(matches!(local.interaction, Interaction::Dragging { .. }));
            assert!(ChessPresentation::on_input(
                &input(target, PointerPhase::Move),
                &view,
                &mut local
            )
            .is_none());
            assert!(ChessPresentation::on_input(
                &input(target, PointerPhase::Up),
                &view,
                &mut local
            )
            .is_none());
            assert_eq!(
                local.interaction,
                Interaction::Selected { square: Square(12) }
            );
            assert!(local.confirmation.is_none());
            assert!(!local.flipped);
            assert!(local.viewer_override().is_none());
            ChessPresentation::on_input(&input(to, PointerPhase::Move), &view, &mut local);
            assert!(matches!(local.interaction, Interaction::Selected { .. }));
        }
    }
    #[test]
    fn promotion_is_disabled_even_if_the_move_remains_legal_in_a_changed_projection() {
        let state = crate::State::from_fen("k7/4P3/8/8/8/8/8/4K3 w - - 0 1").unwrap();
        let mut view = ChessRules::project(&state, Viewer::Seat(SeatId(0)));
        let viewport = Viewport::new(Vec2::new(390.0, 844.0)).unwrap();
        let layout = BoardLayout::from_viewport(viewport);
        let mut local = ChessLocal::default();
        local.set_viewport(viewport);
        for square in [Square(52), Square(60)] {
            ChessPresentation::on_input(
                &input(layout.square_rect(square).unwrap(), PointerPhase::Down),
                &view,
                &mut local,
            );
            ChessPresentation::on_input(
                &input(layout.square_rect(square).unwrap(), PointerPhase::Up),
                &view,
                &mut local,
            );
        }
        assert!(matches!(local.interaction, Interaction::Promotion { .. }));
        view.fullmove_number += 1;
        assert!(super::super::promotion_buttons(&view, &local, layout)
            .iter()
            .filter(|button| button.id() != super::super::PROMOTION_CANCEL_FOCUS_ID)
            .all(|button| !button.is_enabled()));
        assert!(ChessPresentation::on_input(
            &InputEvent::Key {
                key: Key::Enter,
                pressed: true
            },
            &view,
            &mut local
        )
        .is_none());
    }
}
