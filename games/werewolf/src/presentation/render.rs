//! Pure rendering from authorized projection and ephemeral reveal state.

use super::{
    assets, can_select, controls, knowledge_lines, perspective_label, phase_name,
    public_seat_label, rect, reveal_scope, role_name, role_rules, viewer_seat, Action, Ballot,
    Layout, Panel, Phase, Rect, Role, RoleKnowledge, Vec2, View, ViewEvent, WerewolfLocal,
};
use tabula_design::{Color, Positive};
use tabula_presentation::{
    Align, Border, Camera2D, Corners, FrameCtx, Layer, Paint, RenderCmd, RenderList,
    RenderListBuilder, RenderListError, TextStyleToken,
};

pub(super) fn present(
    view: &View,
    local: &WerewolfLocal,
    frame: &FrameCtx,
) -> Result<RenderList, RenderListError> {
    let layout = Layout::new(frame.viewport());
    let theme = frame.theme();
    let art = theme.game_art.werewolf;
    let mut builder = RenderListBuilder::new(Camera2D::default());
    fill(
        &mut builder,
        rect(0.0, 0.0, layout.viewport.x, layout.viewport.y),
        art.page,
        None,
        0.0,
    )?;
    text(
        &mut builder,
        "Ma Sói",
        Vec2::new(layout.content.origin().x, 12.0),
        TextStyleToken::DisplaySm,
        art.ink,
        layout.viewport.x * 0.55,
    )?;
    builder.push(RenderCmd::Text {
        text: format!("Vòng {} · {}", view.round, phase_name(view.phase)),
        at: Vec2::new(layout.viewport.x - layout.content.origin().x, 18.0),
        style: TextStyleToken::LabelLg,
        align: Align::End,
        max_width: Some(
            Positive::new(layout.viewport.x * 0.48)
                .map_err(|_| RenderListError::InvalidGeometry)?,
        ),
        color: art.gold,
        layer: Layer::HUD,
        z: 0,
    })?;
    let remaining = view
        .phase_ends_at
        .0
        .saturating_sub(frame.now_ms())
        .div_ceil(1000);
    text(
        &mut builder,
        &format!("Góc nhìn: {}  ·  {}s", perspective_label(view), remaining),
        Vec2::new(layout.content.origin().x, 52.0),
        TextStyleToken::BodyMd,
        art.muted,
        layout.viewport.x - 32.0,
    )?;
    if !layout.compact || local.panel == Panel::Card {
        draw_card(&mut builder, view, local, frame, layout.card)?;
    }
    if !layout.compact || local.panel == Panel::Table {
        draw_table(&mut builder, view, local, frame, layout)?;
    }
    for control in controls(view, local, frame.viewport()) {
        control.button(&theme).draw(
            &mut builder,
            &theme,
            &local.interaction,
            &local.focus,
            Layer::HUD,
        )?;
        if control.selected && matches!(control.action, Action::Target(_)) {
            // Selection is also named in the button label; never color alone.
            builder.push(RenderCmd::Rect {
                rect: control.rect,
                radii: Corners::uniform(8.0)?,
                fill: None,
                border: Some(Border::new(3.0, theme.color.selected)?),
                layer: Layer::HUD,
                z: 20,
            })?;
        }
    }
    text(
        &mut builder,
        "Mô phỏng cục bộ · Điều khiển mọi ghế",
        Vec2::new(layout.content.origin().x, layout.viewport.y - 48.0),
        TextStyleToken::LabelMd,
        art.muted,
        layout.viewport.x - 32.0,
    )?;
    text(
        &mut builder,
        "Không kết nối mạng · Không chat / voice",
        Vec2::new(layout.content.origin().x, layout.viewport.y - 28.0),
        TextStyleToken::LabelSm,
        art.muted,
        layout.viewport.x - 32.0,
    )?;
    builder.finish()
}

// Helpers retain semantic colors; no palette literal appears in Rust rendering.
fn fill(
    builder: &mut RenderListBuilder,
    rect: Rect,
    color: impl Into<Option<Color>>,
    border: Option<Color>,
    radius: f32,
) -> Result<(), RenderListError> {
    builder.push(RenderCmd::Rect {
        rect,
        radii: Corners::uniform(radius)?,
        fill: color.into().map(Paint::Solid),
        border: border.map(|c| Border::new(2.0, c)).transpose()?,
        layer: Layer::BOARD,
        z: 0,
    })
}
fn text(
    builder: &mut RenderListBuilder,
    text: &str,
    at: Vec2,
    style: TextStyleToken,
    color: Color,
    width: f32,
) -> Result<(), RenderListError> {
    builder.push(RenderCmd::Text {
        text: text.into(),
        at,
        style,
        align: Align::Start,
        max_width: Some(
            Positive::new(width.max(1.0)).map_err(|_| RenderListError::InvalidGeometry)?,
        ),
        color,
        layer: Layer::HUD,
        z: 0,
    })?;
    Ok(())
}
fn centered(
    builder: &mut RenderListBuilder,
    text: &str,
    at: Vec2,
    style: TextStyleToken,
    color: Color,
    width: f32,
) -> Result<(), RenderListError> {
    builder.push(RenderCmd::Text {
        text: text.into(),
        at,
        style,
        align: Align::Center,
        max_width: Some(
            Positive::new(width.max(1.0)).map_err(|_| RenderListError::InvalidGeometry)?,
        ),
        color,
        layer: Layer::HUD,
        z: 0,
    })
}
fn sprite(
    builder: &mut RenderListBuilder,
    asset: tabula_game_api::AssetRef,
    rect: Rect,
    tint: Color,
) -> Result<(), RenderListError> {
    builder.push(RenderCmd::Sprite {
        asset,
        rect,
        tint,
        rotation: 0.0,
        pivot: rect.origin() + rect.size() * 0.5,
        layer: Layer::PIECES,
        z: 0,
    })
}

#[allow(clippy::too_many_lines)] // Closed front/back render branches share one reveal guard and clip boundary.
fn draw_card(
    builder: &mut RenderListBuilder,
    view: &View,
    local: &WerewolfLocal,
    frame: &FrameCtx,
    card: Rect,
) -> Result<(), RenderListError> {
    let art = frame.theme().game_art.werewolf;
    let width = card.size().x;
    let origin = card.origin();
    if let Some(scope) = reveal_scope(view).filter(|_| local.is_revealed(view)) {
        fill(builder, card, art.card, Some(art.gold), 12.0)?;
        builder.push(RenderCmd::PushClip {
            rect: card,
            layer: Layer::HUD,
            z: 0,
        })?;
        let image = rect(origin.x + 8.0, origin.y + 8.0, width - 16.0, width - 16.0);
        sprite(
            builder,
            assets::role_asset(scope.role),
            image,
            art.piece_tint,
        )?;
        let label_y = origin.y + width + 2.0;
        if width >= 220.0 {
            text(
                builder,
                if scope.role.is_wolf() {
                    "PHE MA SÓI"
                } else {
                    "PHE DÂN LÀNG"
                },
                Vec2::new(origin.x + 14.0, label_y),
                TextStyleToken::LabelSm,
                art.card_ink,
                width - 28.0,
            )?;
        }
        text(
            builder,
            role_name(scope.role),
            Vec2::new(
                origin.x + 14.0,
                label_y + if width < 220.0 { 0.0 } else { 17.0 },
            ),
            if width < 220.0 {
                TextStyleToken::TitleMd
            } else {
                TextStyleToken::TitleLg
            },
            art.card_ink,
            width - 28.0,
        )?;
        let short = match scope.role {
            Role::Villager => "Thảo luận. Bỏ phiếu.",
            Role::Werewolf => "Chọn người ngoài phe Sói.",
            Role::Seer => "Soi một người khác.",
            Role::Doctor => "Bảo vệ, không lặp người.",
            Role::Hunter => "Đặt trước mục tiêu.",
            Role::Witch => "Cứu mù / độc.",
        };
        text(
            builder,
            if width < 260.0 {
                short
            } else {
                role_rules(scope.role)
            },
            Vec2::new(
                origin.x + 14.0,
                label_y + if width < 220.0 { 26.0 } else { 46.0 },
            ),
            TextStyleToken::BodySm,
            art.card_ink,
            width - 28.0,
        )?;
        builder.push(RenderCmd::PopClip {
            layer: Layer::HUD,
            z: 0,
        })?;
    } else {
        // Exactly this common back is emitted for all six concealed roles.
        sprite(builder, assets::back_asset(), card, art.piece_tint)?;
        centered(
            builder,
            "TABULA",
            Vec2::new(origin.x + width * 0.5, origin.y + card.size().y * 0.09),
            TextStyleToken::LabelSm,
            art.gold,
            width * 0.8,
        )?;
        centered(
            builder,
            "MA SÓI",
            Vec2::new(origin.x + width * 0.5, origin.y + card.size().y * 0.81),
            if width < 200.0 {
                TextStyleToken::TitleMd
            } else {
                TextStyleToken::TitleLg
            },
            art.gold,
            width * 0.85,
        )?;
        centered(
            builder,
            if width < 220.0 {
                "Giữ bí mật"
            } else {
                "Một đêm. Nhiều bí mật."
            },
            Vec2::new(origin.x + width * 0.5, origin.y + card.size().y * 0.9),
            TextStyleToken::LabelSm,
            art.gold,
            width * 0.85,
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_lines)] // One clipped disclosure surface; all private facts share the explicit-reveal guard.
fn draw_table(
    builder: &mut RenderListBuilder,
    view: &View,
    local: &WerewolfLocal,
    frame: &FrameCtx,
    layout: Layout,
) -> Result<(), RenderListError> {
    let art = frame.theme().game_art.werewolf;
    let table = layout.table;
    text(
        builder,
        "Bàn chơi · Chọn mục tiêu, rồi gửi",
        table.origin(),
        TextStyleToken::TitleSm,
        art.ink,
        table.size().x,
    )?;
    let control_list = controls(view, local, frame.viewport());
    let rows_end = control_list
        .iter()
        .filter(|c| {
            matches!(
                c.action,
                Action::Target(_)
                    | Action::Page
                    | Action::Heal
                    | Action::Poison
                    | Action::Submit
                    | Action::Pass
                    | Action::Unvote
            )
        })
        .map(|c| c.rect.origin().y + c.rect.size().y)
        .fold(table.origin().y + 34.0, f32::max);
    let info_y = rows_end + 14.0;
    let available = (layout.viewport.y - 110.0 - info_y).max(0.0);
    builder.push(RenderCmd::PushClip {
        rect: rect(table.origin().x, info_y, table.size().x, available),
        layer: Layer::HUD,
        z: 0,
    })?;
    let mut lines = Vec::new();
    if let Some(label) = super::outcome_label(view) {
        lines.push(label.into());
    } else if view.phase == Phase::Night && !local.is_revealed(view) && viewer_seat(view).is_some()
    {
        lines.push("Mở lá bài để xem và chọn hành động riêng. Escape để che".into());
    } else if view.phase == Phase::Day {
        lines.push("Thảo luận trực tiếp. Chuyển sang pha Bỏ phiếu khi sẵn sàng".into());
    } else if matches!(view.phase, Phase::Dawn | Phase::Dusk) {
        lines.push("Kết quả đã được xử lý theo luật. Chuyển pha để tiếp tục".into());
    } else if viewer_seat(view).is_none() {
        lines.push("Góc nhìn công khai: không có lá bài riêng hoặc hành động".into());
    }
    if local.is_revealed(view) {
        lines.extend(knowledge_lines(view));
        if let Some(scope) = reveal_scope(view) {
            if view.phase == Phase::Night && scope.role == Role::Villager {
                lines.push("Dân làng không có hành động riêng trong đêm".into());
            }
        }
    }
    if let Some(seat) = local
        .active_selected(view)
        .filter(|_| can_select(view, local))
    {
        lines.push(format!(
            "Đang chọn: Người {} · Chỉ gửi khi bấm nút",
            u16::from(seat.0) + 1
        ));
    }
    if view.phase == Phase::Vote {
        lines.push(format!(
            "{} phiếu công khai. Có thể đổi hoặc rút trước hết pha",
            view.votes.len()
        ));
        for (seat, ballot) in &view.votes {
            lines.push(match ballot {
                Ballot::Target(target) => format!(
                    "Người {} → Người {}",
                    u16::from(seat.0) + 1,
                    u16::from(target.0) + 1
                ),
                Ballot::Abstain => format!("Người {} → Trắng phiếu", u16::from(seat.0) + 1),
            });
        }
    }
    let revealed = local.is_revealed(view);
    for seat in view
        .roster
        .iter()
        .skip(local.page * super::page_size(frame.viewport()))
        .take(super::page_size(frame.viewport()))
    {
        if (!seat.alive || view.phase == Phase::Ended || revealed)
            && matches!(seat.role, RoleKnowledge::Known(_))
        {
            lines.push(public_seat_label(
                seat.seat, seat.alive, seat.role, view.phase, revealed,
            ));
        }
    }
    for event in view.public_history.iter().rev().take(3) {
        match event {
            ViewEvent::DeathRevealed { seat, role } => lines.push(format!(
                "Người {} đã chết · {}",
                u16::from(seat.0) + 1,
                role_name(*role)
            )),
            ViewEvent::VoteResolved {
                eliminated: None, ..
            } => lines.push("Không ai bị loại trong lượt bỏ phiếu".into()),
            _ => {}
        }
    }
    text(
        builder,
        &lines.join("\n"),
        Vec2::new(table.origin().x, info_y),
        TextStyleToken::BodySm,
        art.muted,
        table.size().x,
    )?;
    builder.push(RenderCmd::PopClip {
        layer: Layer::HUD,
        z: 0,
    })?;
    Ok(())
}
