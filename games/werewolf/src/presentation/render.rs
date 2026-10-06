//! Village scene and authorized card art; all interaction geometry is shared (I-5/I-10).
use super::{
    assets, can_select, controls, knowledge_lines, outcome_label, phase_name, public_display_label,
    rect, reveal_scope, role_name, role_rules, viewer_seat, Action, Ballot, Control, Layout, Panel,
    Phase, Rect, Role, Vec2, View, WerewolfLocal,
};
use tabula_design::{Color, Positive};
use tabula_presentation::{
    Align, AvatarFallback, Border, Camera2D, Corners, FrameCtx, Layer, MotionMode, MotionTimeline,
    Paint, PublicAvatar, RenderCmd, RenderList, RenderListBuilder, RenderListError, TextStyleToken,
};

#[allow(clippy::too_many_lines)]
pub(super) fn present(
    view: &View,
    local: &WerewolfLocal,
    frame: &FrameCtx,
) -> Result<RenderList, RenderListError> {
    if !Layout::supports(frame.viewport()) {
        let theme = frame.theme();
        let size = frame.viewport().size();
        let mut b = RenderListBuilder::new(Camera2D::default());
        fill(
            &mut b,
            rect(0.0, 0.0, size.x, size.y),
            theme.color.surface,
            None,
            0.0,
        )?;
        if size.x >= 160.0 && size.y >= 44.0 {
            text(
                &mut b,
                "Mở rộng cửa sổ để tiếp tục",
                Vec2::new(8.0, 8.0),
                TextStyleToken::BodyMd,
                theme.color.on_surface,
                size.x - 16.0,
            )?;
        }
        return b.finish();
    }
    let layout = Layout::new(frame.viewport());
    let theme = frame.theme();
    let art = theme.game_art.werewolf;
    let mut b = RenderListBuilder::new(Camera2D::default());
    fill(
        &mut b,
        rect(0.0, 0.0, layout.viewport.x, layout.viewport.y),
        art.page,
        None,
        0.0,
    )?;
    let title = match view.phase {
        Phase::Night => "Đêm ở làng Sương",
        Phase::Dawn => "Bình minh trong làng",
        Phase::Day => "Làng mở lời",
        Phase::Vote => "Ai là Ma Sói?",
        Phase::Dusk => "Hoàng hôn xuống",
        Phase::Ended => "Chuyện làng đã khép",
    };
    text(
        &mut b,
        "MA SÓI · MÔ PHỎNG CỤC BỘ",
        Vec2::new(layout.content.origin().x, 8.0),
        TextStyleToken::LabelSm,
        art.gold,
        layout.viewport.x - 24.0,
    )?;
    text(
        &mut b,
        title,
        Vec2::new(layout.content.origin().x, 28.0),
        if layout.compact {
            TextStyleToken::TitleMd
        } else {
            TextStyleToken::DisplaySm
        },
        art.ink,
        layout.viewport.x - 24.0,
    )?;
    if layout.viewport.y >= 500.0 {
        let remaining = view
            .phase_ends_at
            .0
            .saturating_sub(frame.now_ms())
            .div_ceil(1000);
        text(
            &mut b,
            &format!(
                "Vòng {} · {} · {}s · {}",
                view.round,
                phase_name(view.phase),
                remaining,
                super::perspective_label(view)
            ),
            Vec2::new(layout.content.origin().x, 69.0),
            TextStyleToken::LabelMd,
            art.muted,
            layout.viewport.x - 24.0,
        )?;
    }
    draw_village(&mut b, view, local, frame, layout)?;
    if layout.compact {
        sprite(
            &mut b,
            assets::back_asset(),
            rect(
                layout.reveal.origin().x + 8.0,
                layout.reveal.origin().y + 4.0,
                24.0,
                36.0,
            ),
            art.piece_tint,
        )?;
    }
    if !layout.compact {
        text(
            &mut b,
            "LÁ BÀI CỦA BẠN",
            Vec2::new(layout.card.origin().x, layout.table.origin().y + 4.0),
            TextStyleToken::LabelSm,
            art.muted,
            layout.card.size().x,
        )?;
        draw_card(&mut b, view, local, frame, layout.card)?;
        draw_private_info(&mut b, view, local, frame, layout)?;
    }
    fill(
        &mut b,
        layout.dock,
        theme.color.surface_container,
        Some(theme.color.outline),
        18.0,
    )?;
    if !can_select(view, local) || (!local.is_revealed(view) && view.phase != Phase::Vote) {
        let hint = if let Some(result) = outcome_label(view) {
            result
        } else if viewer_seat(view).is_none() {
            if layout.compact {
                "Công khai · Không có hành động"
            } else {
                "Góc nhìn công khai · Không có hành động"
            }
        } else if view.phase == Phase::Night {
            "Mở bài riêng để chọn hành động"
        } else {
            "Lắng nghe làng · Chờ pha tiếp theo"
        };
        let public_notice = current_death_notice(view, local);
        let hint = if matches!(view.phase, Phase::Dawn | Phase::Dusk) {
            public_notice.as_deref().unwrap_or(hint)
        } else {
            hint
        };
        let bounds = rect(
            layout.dock.origin().x + 12.0,
            layout.dock.origin().y + 8.0,
            layout.dock.size().x - 24.0,
            40.0,
        );
        b.push(RenderCmd::PushClip {
            rect: bounds,
            layer: Layer::HUD,
            z: 0,
        })?;
        text(
            &mut b,
            hint,
            bounds.origin(),
            if layout.compact {
                TextStyleToken::LabelMd
            } else {
                TextStyleToken::BodyMd
            },
            theme.color.on_surface,
            bounds.size().x,
        )?;
        b.push(RenderCmd::PopClip {
            layer: Layer::HUD,
            z: 0,
        })?;
    } else if !reveal_scope(view).is_some_and(|s| {
        local.is_revealed(view) && view.phase == Phase::Night && s.role == super::Role::Witch
    }) {
        let (hint, bounds, style) = dock_selection_hint(view, local, layout);
        text(
            &mut b,
            &hint,
            bounds.origin(),
            style,
            theme.color.on_surface,
            bounds.size().x,
        )?;
    }
    for c in controls(view, local, frame.viewport()) {
        if !(matches!(c.action, Action::Target(_))
            || local.panel == Panel::Tools
            || layout.compact && local.panel == Panel::Card)
        {
            draw_control(&mut b, &c, local, frame)?;
        }
    }
    if local.panel == Panel::Tools || (layout.compact && local.panel == Panel::Card) {
        b.push(RenderCmd::PushClip {
            rect: rect(0.0, 0.0, layout.viewport.x, layout.viewport.y),
            layer: Layer::MODAL,
            z: 0,
        })?;
        fill(
            &mut b,
            rect(0.0, 0.0, layout.viewport.x, layout.viewport.y),
            alpha(
                theme.color.surface,
                if local.panel == Panel::Tools {
                    0.82
                } else {
                    0.98
                },
            ),
            None,
            0.0,
        )?;
        if local.panel == Panel::Tools {
            draw_options(&mut b, frame, layout)?;
        } else {
            text(
                &mut b,
                "Bài riêng · Escape để che ngay",
                Vec2::new(16.0, 12.0),
                TextStyleToken::LabelLg,
                theme.color.on_surface,
                layout.viewport.x - 32.0,
            )?;
            draw_card(&mut b, view, local, frame, layout.card)?;
            if local.is_revealed(view) {
                let x = if layout.landscape {
                    layout.card.origin().x + layout.card.size().x + 24.0
                } else {
                    16.0
                };
                let y = if layout.landscape {
                    170.0
                } else {
                    layout.card.origin().y + layout.card.size().y + 116.0
                };
                let bounds = rect(
                    x,
                    y,
                    layout.viewport.x - x - 16.0,
                    (layout.viewport.y - y - 56.0).max(0.0),
                );
                b.push(RenderCmd::PushClip {
                    rect: bounds,
                    layer: Layer::HUD,
                    z: 2,
                })?;
                let lines = knowledge_lines(view)
                    .into_iter()
                    .skip(local.info_page * 3)
                    .take(3)
                    .collect::<Vec<_>>()
                    .join("\n");
                text(
                    &mut b,
                    &lines,
                    bounds.origin(),
                    TextStyleToken::BodySm,
                    theme.color.on_surface,
                    bounds.size().x,
                )?;
                b.push(RenderCmd::PopClip {
                    layer: Layer::HUD,
                    z: 2,
                })?;
            }
        }
        for c in controls(view, local, frame.viewport()) {
            draw_control(&mut b, &c, local, frame)?;
        }
        b.push(RenderCmd::PopClip {
            layer: Layer::MODAL,
            z: 0,
        })?;
    }
    b.finish()
}

fn draw_control(
    b: &mut RenderListBuilder,
    control: &Control,
    local: &WerewolfLocal,
    frame: &FrameCtx,
) -> Result<(), RenderListError> {
    let theme = frame.theme();
    if control.action != Action::Tools || local.panel == Panel::Tools {
        return control.button(&theme).draw(
            b,
            &theme,
            &local.interaction,
            &local.focus,
            Layer::HUD,
        );
    }
    // Retain the shared button's exact hit/focus/feedback geometry. The small
    // slider mark is drawn from primitives, without an icon-font dependency.
    tabula_presentation::ActionButton::new(control.id, control.rect, "", theme.density.min_target)?
        .draw(b, &theme, &local.interaction, &local.focus, Layer::HUD)?;
    b.push(RenderCmd::Rect {
        rect: control.rect,
        radii: Corners::uniform(22.0)?,
        fill: None,
        border: Some(Border::new(1.0, theme.color.outline)?),
        layer: Layer::HUD,
        z: 0,
    })?;
    let origin = control.rect.origin();
    for (row, knob) in [(0.0, 3.0), (6.0, 10.0), (12.0, 6.0)] {
        for (bounds, radius) in [
            (
                rect(origin.x + 16.0, origin.y + 15.0 + row, 16.0, 1.5),
                0.75,
            ),
            (
                rect(origin.x + 16.0 + knob, origin.y + 13.5 + row, 4.0, 4.0),
                2.0,
            ),
        ] {
            b.push(RenderCmd::Rect {
                rect: bounds,
                radii: Corners::uniform(radius)?,
                fill: Some(Paint::Solid(theme.color.on_surface)),
                border: None,
                layer: Layer::HUD,
                z: 0,
            })?;
        }
    }
    text(
        b,
        &control.label,
        Vec2::new(
            origin.x + 42.0,
            origin.y
                + (control.rect.size().y
                    - theme
                        .text_style(TextStyleToken::LabelLg)
                        .line_height()
                        .get())
                    * 0.5,
        ),
        TextStyleToken::LabelLg,
        theme.color.on_surface,
        control.rect.size().x - 50.0,
    )
}

fn draw_options(
    b: &mut RenderListBuilder,
    frame: &FrameCtx,
    layout: Layout,
) -> Result<(), RenderListError> {
    let theme = frame.theme();
    let options = layout.options();
    let dialog = options.dialog;
    let origin = dialog.origin();
    fill(
        b,
        dialog,
        theme.color.surface,
        Some(theme.color.outline),
        24.0,
    )?;
    text(
        b,
        "Tùy chọn",
        origin + Vec2::new(20.0, 12.0),
        TextStyleToken::TitleLg,
        theme.color.on_surface,
        dialog.size().x - 40.0,
    )?;
    if !options.condensed {
        text(
            b,
            "Thử game trên một máy",
            origin + Vec2::new(20.0, 52.0),
            TextStyleToken::BodySm,
            theme.color.on_surface_variant,
            dialog.size().x - 40.0,
        )?;
    }
    fill(b, options.motion, theme.color.surface_container, None, 16.0)?;
    text(
        b,
        "Chuyển động",
        options.motion.origin() + Vec2::new(12.0, 16.0),
        TextStyleToken::LabelLg,
        theme.color.on_surface,
        options.motion.size().x - 104.0,
    )?;
    if !options.condensed {
        text(
            b,
            "Lật bài · Đổi cảnh · Phiếu",
            options.motion.origin() + Vec2::new(12.0, 58.0),
            TextStyleToken::LabelSm,
            theme.color.on_surface_variant,
            options.motion.size().x - 24.0,
        )?;
    }
    text(
        b,
        "Công cụ mô phỏng",
        Vec2::new(
            origin.x + 20.0,
            options.tools_y - if options.condensed { 24.0 } else { 28.0 },
        ),
        TextStyleToken::LabelMd,
        theme.color.on_surface_variant,
        dialog.size().x - 40.0,
    )?;
    Ok(())
}

pub(super) fn dock_selection_hint(
    view: &View,
    local: &WerewolfLocal,
    layout: Layout,
) -> (String, Rect, TextStyleToken) {
    let voting = view.phase == Phase::Vote;
    let reserved = if voting && view.legal_commands.contains(&super::Command::Unvote) {
        112.0
    } else {
        0.0
    };
    let hint = local.active_selected(view).map_or_else(
        || {
            if voting {
                "Chọn người bỏ phiếu"
            } else {
                "Chọn chân dung, rồi xác nhận"
            }
            .into()
        },
        |seat| {
            let label = public_display_label(local, seat);
            if voting && layout.compact {
                let short: String = label.chars().take(10).collect();
                format!("Đã chọn {short}")
            } else {
                format!("Đã chọn {label} · Chưa gửi")
            }
        },
    );
    (
        hint,
        rect(
            layout.dock.origin().x + 12.0,
            layout.dock.origin().y + 8.0,
            (layout.dock.size().x - 24.0 - reserved).clamp(1.0, 650.0),
            40.0,
        ),
        if layout.compact {
            TextStyleToken::LabelMd
        } else {
            TextStyleToken::BodyMd
        },
    )
}

/// Only deaths inside this round's preceding Night/Vote → Dawn/Dusk batch.
/// A later deathless announcement must not replay an older round's history.
pub(super) fn current_death_notice(view: &View, local: &WerewolfLocal) -> Option<String> {
    let preceding_phase = match view.phase {
        Phase::Dawn => Phase::Night,
        Phase::Dusk => Phase::Vote,
        _ => return None,
    };
    let end = view
        .public_history
        .iter()
        .rposition(|event| matches!(event, super::ViewEvent::PhaseChanged { .. }))?;
    if !matches!(view.public_history[end], super::ViewEvent::PhaseChanged { phase, round, .. } if phase == view.phase && round == view.round)
    {
        return None;
    }
    let start = view.public_history[..end]
        .iter()
        .rposition(|event| matches!(event, super::ViewEvent::PhaseChanged { .. }))?;
    if !matches!(view.public_history[start], super::ViewEvent::PhaseChanged { phase, round, .. } if phase == preceding_phase && round == view.round)
    {
        return None;
    }
    let deaths: Vec<_> = view.public_history[start + 1..end]
        .iter()
        .filter_map(|event| match event {
            super::ViewEvent::DeathRevealed { seat, role } => Some((*seat, *role)),
            _ => None,
        })
        .take(20)
        .collect();
    match deaths.as_slice() {
        [] => None,
        [(seat, role)] => Some(format!(
            "{} đã bị loại · {}",
            public_display_label(local, *seat),
            role_name(*role)
        )),
        many => Some(format!("{} người đã bị loại · Làng tưởng niệm", many.len())),
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn alpha(color: Color, a: f32) -> Color {
    color.with_alpha((a.clamp(0.0, 1.0) * 255.0) as u8)
}
fn fill(
    b: &mut RenderListBuilder,
    bounds: Rect,
    color: Color,
    border: Option<Color>,
    radius: f32,
) -> Result<(), RenderListError> {
    b.push(RenderCmd::Rect {
        rect: bounds,
        radii: Corners::uniform(radius)?,
        fill: Some(Paint::Solid(color)),
        border: border.map(|c| Border::new(2.0, c)).transpose()?,
        layer: Layer::BOARD,
        z: 1,
    })
}
fn text(
    b: &mut RenderListBuilder,
    value: &str,
    at: Vec2,
    style: TextStyleToken,
    color: Color,
    width: f32,
) -> Result<(), RenderListError> {
    b.push(RenderCmd::Text {
        text: value.into(),
        at,
        style,
        align: Align::Start,
        max_width: Some(
            Positive::new(width.max(1.0)).map_err(|_| RenderListError::InvalidGeometry)?,
        ),
        color,
        layer: Layer::HUD,
        z: 0,
    })
}
fn centered(
    b: &mut RenderListBuilder,
    value: &str,
    at: Vec2,
    style: TextStyleToken,
    color: Color,
    width: f32,
) -> Result<(), RenderListError> {
    b.push(RenderCmd::Text {
        text: value.into(),
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
    b: &mut RenderListBuilder,
    asset: tabula_game_api::AssetRef,
    bounds: Rect,
    tint: Color,
) -> Result<(), RenderListError> {
    b.push(RenderCmd::Sprite {
        asset,
        rect: bounds,
        tint,
        rotation: 0.0,
        pivot: bounds.origin() + bounds.size() * 0.5,
        layer: Layer::PIECES,
        z: 0,
    })
}
fn factor(
    start: Option<u64>,
    profile: tabula_design::MotionProfile,
    local: &WerewolfLocal,
    frame: &FrameCtx,
) -> f32 {
    if local.reduced_motion || !local.focus.is_window_focused() {
        return 1.0;
    }
    start.map_or(1.0, |s| {
        MotionTimeline::from_profile(s, profile, &frame.theme(), MotionMode::Full)
            .sample(frame.now_ms())
            .factor
            .clamp(0.0, 1.0)
    })
}

#[allow(clippy::too_many_lines)]
fn draw_village(
    b: &mut RenderListBuilder,
    view: &View,
    local: &WerewolfLocal,
    frame: &FrameCtx,
    layout: Layout,
) -> Result<(), RenderListError> {
    let theme = frame.theme();
    let art = theme.game_art.werewolf;
    let table = layout.table;
    b.push(RenderCmd::PushClip {
        rect: table,
        layer: Layer::BOARD,
        z: 2,
    })?;
    let night = matches!(view.phase, Phase::Night | Phase::Dusk);
    if let Some((phase, start)) = local
        .phase_motion
        .filter(|(p, _)| *p == view.phase && matches!(p, Phase::Dawn | Phase::Dusk))
    {
        let progress = factor(Some(start), theme.motion.phase_change, local, frame);
        if progress < 1.0 {
            sprite(
                b,
                assets::scene_asset(!matches!(phase, Phase::Night | Phase::Dusk)),
                table,
                art.piece_tint,
            )?;
        }
        sprite(
            b,
            assets::scene_asset(night),
            table,
            alpha(art.piece_tint, progress),
        )?;
    } else {
        sprite(b, assets::scene_asset(night), table, art.piece_tint)?;
    }
    b.push(RenderCmd::Rect {
        rect: table,
        radii: Corners::uniform(0.0)?,
        fill: Some(Paint::Solid(alpha(
            art.page,
            if layout.compact { 0.62 } else { 0.30 },
        ))),
        border: Some(Border::new(1.0, theme.color.outline)?),
        layer: Layer::OVERLAY,
        z: 0,
    })?;
    b.push(RenderCmd::PopClip {
        layer: Layer::BOARD,
        z: 2,
    })?;
    text(
        b,
        "LÀNG SƯƠNG",
        table.origin() + Vec2::new(12.0, 8.0),
        TextStyleToken::LabelSm,
        art.gold,
        table.size().x - 24.0,
    )?;
    if !layout.compact || table.size().y >= 330.0 {
        let at = if layout.compact {
            table.origin() + Vec2::new(table.size().x * 0.5, 36.0)
        } else {
            table.origin() + table.size() * 0.5 - Vec2::new(0.0, 38.0)
        };
        centered(
            b,
            outcome_label(view).unwrap_or(if night {
                "Làng đang ngủ"
            } else {
                "Làng thức giấc"
            }),
            at,
            TextStyleToken::TitleMd,
            art.ink,
            table.size().x * 0.65,
        )?;
        if view.outcome.is_some() {
            let start = local
                .win_motion
                .as_ref()
                .filter(|(outcome, _)| Some(outcome) == view.outcome.as_ref())
                .map(|(_, start)| *start);
            let progress = factor(start, theme.motion.win, local, frame);
            let width = table.size().x * 0.50 * (0.25 + 0.75 * progress);
            b.push(RenderCmd::Rect {
                rect: rect(at.x - width * 0.5, at.y + 30.0, width, 3.0),
                radii: Corners::uniform(1.5)?,
                fill: Some(Paint::Solid(alpha(art.gold, 0.30 + 0.70 * progress))),
                border: None,
                layer: Layer::HUD,
                z: 4,
            })?;
        }
    }
    let start = local.page * super::page_size(frame.viewport());
    let seats: Vec<_> = view
        .roster
        .iter()
        .skip(start)
        .take(super::page_size(frame.viewport()))
        .collect();
    for (i, seat) in seats.iter().enumerate() {
        let bounds = layout.seat_rect(i, seats.len());
        let selected = local.active_selected(view) == Some(seat.seat) && can_select(view, local);
        let enabled = super::target_command(view, local, seat.seat).is_some();
        let id = tabula_presentation::FocusId::new(100 + u32::from(seat.seat.0));
        let focused = local.focus.current() == Some(id) && local.focus.is_focus_visible();
        let d = (bounds.size().x - 12.0)
            .min(bounds.size().y - 40.0)
            .clamp(18.0, 64.0);
        let lift = if selected {
            3.0 * factor(local.selected_started_ms, theme.motion.vote, local, frame)
        } else {
            0.0
        };
        let portrait = rect(
            bounds.origin().x + (bounds.size().x - d) * 0.5,
            bounds.origin().y + 2.0 - lift,
            d,
            d,
        );
        b.push(RenderCmd::Rect {
            rect: portrait,
            radii: Corners::uniform(d * 0.5)?,
            fill: Some(Paint::Solid(theme.color.surface_container_high)),
            border: Some(Border::new(
                if focused || selected { 3.0 } else { 1.5 },
                if focused {
                    theme.color.primary
                } else if selected {
                    theme.color.selected
                } else if enabled {
                    art.gold
                } else {
                    theme.color.outline
                },
            )?),
            layer: Layer::HUD,
            z: 2,
        })?;
        let avatar = local.public_display.get(seat.seat).map_or(
            PublicAvatar::Fallback(AvatarFallback::Human),
            tabula_presentation::PublicDisplay::avatar,
        );
        let portrait_opacity = if seat.alive {
            1.0
        } else {
            tabula_presentation::lerp_f32(
                1.0,
                0.45,
                factor(
                    local.death_motion.get(&seat.seat).copied(),
                    theme.motion.exit,
                    local,
                    frame,
                ),
            )
        };
        match avatar {
            PublicAvatar::Image(asset) => {
                b.push(RenderCmd::Sprite {
                    asset: asset.clone(),
                    rect: portrait,
                    tint: alpha(art.piece_tint, portrait_opacity),
                    rotation: 0.0,
                    pivot: portrait.origin() + portrait.size() * 0.5,
                    layer: Layer::HUD,
                    z: 3,
                })?;
            }
            PublicAvatar::Fallback(kind) => {
                let ink = alpha(theme.color.on_surface_variant, portrait_opacity);
                if kind == AvatarFallback::Human {
                    b.push(RenderCmd::Rect {
                        rect: rect(
                            portrait.origin().x + d * 0.36,
                            portrait.origin().y + d * 0.20,
                            d * 0.28,
                            d * 0.28,
                        ),
                        radii: Corners::uniform(d * 0.14)?,
                        fill: Some(Paint::Solid(ink)),
                        border: None,
                        layer: Layer::HUD,
                        z: 3,
                    })?;
                    b.push(RenderCmd::Rect {
                        rect: rect(
                            portrait.origin().x + d * 0.23,
                            portrait.origin().y + d * 0.53,
                            d * 0.54,
                            d * 0.25,
                        ),
                        radii: Corners::uniform(d * 0.12)?,
                        fill: Some(Paint::Solid(ink)),
                        border: None,
                        layer: Layer::HUD,
                        z: 3,
                    })?;
                } else {
                    centered(
                        b,
                        kind.glyph(),
                        portrait.origin() + Vec2::new(d * 0.5, d * 0.2),
                        TextStyleToken::TitleMd,
                        ink,
                        d,
                    )?;
                }
            }
        }
        // The public image never covers the target/focus affordance.
        b.push(RenderCmd::Rect {
            rect: portrait,
            radii: Corners::uniform(d * 0.5)?,
            fill: None,
            border: Some(Border::new(
                if focused || selected { 3.0 } else { 1.5 },
                if focused {
                    theme.color.primary
                } else if selected {
                    theme.color.selected
                } else if enabled {
                    art.gold
                } else {
                    theme.color.outline
                },
            )?),
            layer: Layer::HUD,
            z: 4,
        })?;
        if !local.deal_cancelled && !local.reduced_motion && local.focus.is_window_focused() {
            let start = local.started_at_ms.map(|t| {
                tabula_presentation::staggered_start(
                    t,
                    u32::try_from(i).expect("12 seats"),
                    theme.motion.card_deal.stagger,
                )
            });
            let progress = factor(start, theme.motion.card_deal, local, frame);
            if progress < 1.0 {
                let pos = tabula_presentation::lerp_vec2(
                    table.origin() + table.size() * 0.5,
                    portrait.origin() + Vec2::new(d * 0.6, 0.0),
                    progress,
                );
                b.push(RenderCmd::Sprite {
                    asset: assets::back_asset(),
                    rect: rect(pos.x, pos.y, 16.0, 24.0),
                    tint: art.piece_tint,
                    rotation: 0.0,
                    pivot: pos,
                    layer: Layer::HUD,
                    z: 5,
                })?;
            }
        }
        let label_y = portrait.origin().y + d + 2.0;
        let label = public_display_label(local, seat.seat);
        let max_chars: usize = if bounds.size().x < 75.0 { 9 } else { 12 };
        let mut short: String = label.chars().take(max_chars.saturating_sub(1)).collect();
        if label.chars().count() >= max_chars {
            short.push('…');
        }
        b.push(RenderCmd::Rect {
            rect: rect(
                bounds.origin().x + 1.0,
                label_y,
                bounds.size().x - 2.0,
                17.0,
            ),
            radii: Corners::uniform(6.0)?,
            fill: Some(Paint::Solid(alpha(theme.color.surface, 0.96))),
            border: None,
            layer: Layer::HUD,
            z: 0,
        })?;
        centered(
            b,
            &short,
            Vec2::new(bounds.origin().x + bounds.size().x * 0.5, label_y),
            TextStyleToken::LabelMd,
            theme.color.on_surface,
            bounds.size().x - 4.0,
        )?;
        centered(
            b,
            if (!seat.alive || view.phase == Phase::Ended)
                && matches!(seat.role, super::RoleKnowledge::Known(_))
            {
                match seat.role {
                    super::RoleKnowledge::Known(role) => role_name(role),
                    super::RoleKnowledge::Hidden => "Đã bị loại",
                }
            } else if selected {
                "Đã chọn"
            } else if !seat.alive {
                "Đã bị loại"
            } else if viewer_seat(view) == Some(seat.seat) {
                "Bạn"
            } else {
                "Còn sống"
            },
            Vec2::new(bounds.origin().x + bounds.size().x * 0.5, label_y + 17.0),
            TextStyleToken::LabelMd,
            if selected {
                theme.color.selected
            } else {
                art.ink
            },
            bounds.size().x,
        )?;
        if !seat.alive {
            draw_public_badge(
                b,
                "Loại",
                rect(
                    portrait.origin().x + d - 24.0,
                    portrait.origin().y,
                    28.0,
                    18.0,
                ),
                theme.color.surface_container_high,
                theme.color.on_surface,
            )?;
        }
        let votes = view
            .votes
            .values()
            .filter(|ballot| **ballot == Ballot::Target(seat.seat))
            .count();
        if view.phase == Phase::Vote && votes > 0 {
            draw_public_badge(
                b,
                &format!("{votes}"),
                rect(
                    portrait.origin().x + d - 18.0,
                    portrait.origin().y,
                    24.0,
                    24.0,
                ),
                theme.color.primary,
                theme.color.on_primary,
            )?;
        }
    }
    if !local.reduced_motion && local.focus.is_window_focused() {
        let center = if layout.compact {
            table.origin() + Vec2::new(table.size().x * 0.5, 66.0)
        } else {
            table.origin() + table.size() * 0.5 + Vec2::new(0.0, 34.0)
        };
        for i in 0..12_u16 {
            let t = f32::from(
                u16::try_from((frame.now_ms().saturating_add(u64::from(i) * 173)) % 1600)
                    .expect("bounded particle cycle"),
            ) / 1600.0;
            b.push(RenderCmd::Rect {
                rect: rect(
                    center.x + (f32::from(i) - 5.5) * 2.5,
                    center.y - t * 20.0,
                    2.0,
                    2.0,
                ),
                radii: Corners::uniform(1.0)?,
                fill: Some(Paint::Solid(alpha(art.gold, 1.0 - t))),
                border: None,
                layer: Layer::OVERLAY,
                z: 1,
            })?;
        }
    }
    if let Some((from, to, start)) = local.vote_motion.filter(|_| view.phase == Phase::Vote) {
        let progress = factor(Some(start), theme.motion.vote, local, frame);
        if progress < 1.0 {
            if let (Some(a), Some(z)) = (
                seats.iter().position(|s| s.seat == from),
                seats.iter().position(|s| s.seat == to),
            ) {
                let a = layout.seat_rect(a, seats.len());
                let z = layout.seat_rect(z, seats.len());
                let pos = tabula_presentation::lerp_vec2(
                    a.origin() + a.size() * 0.5,
                    z.origin() + z.size() * 0.5,
                    progress,
                );
                b.push(RenderCmd::Rect {
                    rect: rect(pos.x, pos.y, 8.0, 8.0),
                    radii: Corners::uniform(4.0)?,
                    fill: Some(Paint::Solid(theme.color.primary)),
                    border: None,
                    layer: Layer::HUD,
                    z: 4,
                })?;
            }
        }
    }
    Ok(())
}

fn draw_public_badge(
    b: &mut RenderListBuilder,
    label: &str,
    bounds: Rect,
    fill: Color,
    ink: Color,
) -> Result<(), RenderListError> {
    b.push(RenderCmd::Rect {
        rect: bounds,
        radii: Corners::uniform(8.0)?,
        fill: Some(Paint::Solid(fill)),
        border: None,
        layer: Layer::HUD,
        z: 5,
    })?;
    b.push(RenderCmd::Text {
        text: label.into(),
        at: bounds.origin() + Vec2::new(bounds.size().x * 0.5, 1.0),
        style: TextStyleToken::LabelMd,
        align: Align::Center,
        max_width: Some(
            Positive::new(bounds.size().x).map_err(|_| RenderListError::InvalidGeometry)?,
        ),
        color: ink,
        layer: Layer::HUD,
        z: 6,
    })
}
fn draw_private_info(
    b: &mut RenderListBuilder,
    view: &View,
    local: &WerewolfLocal,
    frame: &FrameCtx,
    layout: Layout,
) -> Result<(), RenderListError> {
    let y = layout.reveal.origin().y + layout.reveal.size().y + 12.0;
    let h = (layout.table.origin().y + layout.table.size().y - y).max(0.0);
    if h < 30.0 {
        return Ok(());
    }
    let box_rect = rect(layout.card.origin().x, y, layout.card.size().x, h);
    fill(
        b,
        box_rect,
        frame.theme().color.surface_container,
        None,
        14.0,
    )?;
    b.push(RenderCmd::PushClip {
        rect: box_rect,
        layer: Layer::HUD,
        z: 1,
    })?;
    let mut lines = vec!["Lời của quản trò".to_owned()];
    if local.is_revealed(view) {
        lines.extend(knowledge_lines(view));
    } else {
        lines.push("Chỉ bài của bạn được lật. Vai các ghế khác vẫn được che.".into());
    }
    text(
        b,
        &lines.join("\n"),
        box_rect.origin() + Vec2::new(10.0, 8.0),
        TextStyleToken::BodySm,
        frame.theme().color.on_surface,
        box_rect.size().x - 20.0,
    )?;
    b.push(RenderCmd::PopClip {
        layer: Layer::HUD,
        z: 1,
    })?;
    Ok(())
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
        let progress = factor(
            local.reveal_started_ms,
            frame.theme().motion.reveal,
            local,
            frame,
        );
        let scale = (progress * 2.0 - 1.0).abs().max(0.08);
        let iw = (width - 16.0) * scale;
        let image = rect(
            origin.x + (width - iw) * 0.5,
            origin.y + 8.0,
            iw,
            width - 16.0,
        );
        sprite(
            builder,
            if progress < 0.5 {
                assets::back_asset()
            } else {
                assets::role_asset(scope.role)
            },
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
