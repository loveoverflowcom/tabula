//! Privacy, intent and bounded-layout examples. RenderList checks are not pixels.

use super::*;
use crate::rules::SeatView;
use crate::{Config, WitchPotions};
use std::collections::BTreeMap;
use tabula_core::{LogicalTime, MatchSeed, Occupant, SeatEntry, SeatRoster, UserId};
use tabula_game_api::{Budget, Ctx, GameRules, Input};
use tabula_presentation::{Dpi, PointerButton, PointerPhase, PointerPosition, RenderCmd};

fn frame(w: f32, h: f32) -> FrameCtx {
    FrameCtx::new(
        Viewport::new(Vec2::new(w, h)).unwrap(),
        Dpi::new(1.0).unwrap(),
        1000,
        Theme::by_kind(tabula_design::ThemeKind::Dark),
    )
}
fn view(role: Role) -> View {
    let mut view = View {
        phase: Phase::Night,
        round: 1,
        phase_ends_at: LogicalTime(30_000),
        roster: (0..12)
            .map(|id| SeatView {
                seat: SeatId(id),
                alive: true,
                status: PlayerStatus::Active,
                role: if id == 0 {
                    RoleKnowledge::Known(role)
                } else {
                    RoleKnowledge::Hidden
                },
            })
            .collect(),
        perspective: Perspective::Seat {
            seat: SeatId(0),
            role,
            alive: true,
            can_act: true,
        },
        knowledge: PrivateKnowledge::Living {
            choice: None,
            wolf_team: if role == Role::Werewolf {
                vec![SeatId(0), SeatId(2)]
            } else {
                vec![]
            },
            seer_reports: BTreeMap::new(),
            doctor_previous: None,
            hunter_mark: None,
            witch_potions: (role == Role::Witch).then_some(WitchPotions::default()),
        },
        legal_commands: vec![Command::Night(NightChoice::Pass)],
        votes: BTreeMap::new(),
        public_history: vec![],
        outcome: None,
    };
    let choice = match role {
        Role::Werewolf => Some(NightChoice::WolfTarget(Some(SeatId(1)))),
        Role::Seer => Some(NightChoice::Investigate(SeatId(1))),
        Role::Doctor => Some(NightChoice::Protect(Some(SeatId(1)))),
        Role::Hunter => Some(NightChoice::HunterMark(Some(SeatId(1)))),
        Role::Witch => Some(NightChoice::WitchHeal(Some(SeatId(1)))),
        Role::Villager => None,
    };
    if let Some(choice) = choice {
        view.legal_commands.push(Command::Night(choice));
    }
    view
}
fn click(local: &mut WerewolfLocal, view: &View, action: Action) -> Option<Intent<Command>> {
    if matches!(
        action,
        Action::PreviousSeat
            | Action::NextSeat
            | Action::Public
            | Action::Advance
            | Action::Restart
    ) && local.panel != Panel::Tools
    {
        click(local, view, Action::Tools);
    }
    if matches!(
        action,
        Action::Target(_) | Action::Submit | Action::Pass | Action::Heal | Action::Poison
    ) && local.panel == Panel::Card
    {
        click(local, view, Action::Table);
    }
    if let Action::Target(seat) = action {
        let index = view
            .roster
            .iter()
            .position(|entry| entry.seat == seat)
            .unwrap();
        local.page = index / page_size(local.viewport);
    }
    let control = controls(view, local, local.viewport)
        .into_iter()
        .find(|c| c.action == action)
        .expect("requested control is present");
    let position = PointerPosition::new(control.rect.origin() + control.rect.size() * 0.5).unwrap();
    WerewolfPresentation::on_input(
        &InputEvent::Pointer {
            position,
            button: PointerButton::Primary,
            phase: PointerPhase::Down,
        },
        view,
        local,
    );
    let intent = WerewolfPresentation::on_input(
        &InputEvent::Pointer {
            position,
            button: PointerButton::Primary,
            phase: PointerPhase::Up,
        },
        view,
        local,
    );
    if action == Action::Reveal {
        local.reveal_started_ms = None;
    }
    intent
}
fn assets(list: &RenderList) -> Vec<&str> {
    let mut assets: Vec<_> = list
        .commands()
        .iter()
        .filter_map(|c| match c {
            RenderCmd::Sprite { asset, .. } if asset.as_str().starts_with("cards/") => {
                Some(asset.as_str())
            }
            _ => None,
        })
        .collect();
    assets.sort_unstable();
    assets.dedup();
    if assets.iter().any(|a| *a != "cards/back") {
        assets.retain(|a| *a != "cards/back");
    }
    assets
}
#[test]
fn concealed_render_and_accessibility_are_identical_for_all_six_roles() {
    let f = frame(1024.0, 768.0);
    let mut local = WerewolfLocal::default();
    local.set_frame_context(&f);
    let baseline = WerewolfPresentation::present(&view(Role::Villager), &local, &f);
    let a11y = format!(
        "{:?}",
        WerewolfPresentation::a11y(&view(Role::Villager), &local)
    );
    for role in Role::ALL {
        let v = view(role);
        let list = WerewolfPresentation::present(&v, &local, &f);
        assert_eq!(
            list.commands(),
            baseline.commands(),
            "concealed {role:?} differs"
        );
        assert_eq!(
            format!("{:?}", WerewolfPresentation::a11y(&v, &local)),
            a11y
        );
        assert_eq!(assets(&list), vec!["cards/back"]);
    }
}
#[test]
fn explicit_reveal_emits_only_own_art_and_live_vietnamese_text_then_escape_conceals() {
    for role in Role::ALL {
        let v = view(role);
        let f = frame(390.0, 844.0);
        let mut local = WerewolfLocal::default();
        local.set_frame_context(&f);
        assert!(click(&mut local, &v, Action::Reveal).is_none());
        let list = WerewolfPresentation::present(&v, &local, &f);
        assert_eq!(assets(&list), vec![assets::role_asset(role).as_str()]);
        assert!(list
            .commands()
            .iter()
            .any(|c| matches!(c,RenderCmd::Text {text,..} if text==role_name(role))));
        WerewolfPresentation::on_input(
            &InputEvent::Key {
                key: Key::Escape,
                pressed: true,
            },
            &v,
            &mut local,
        );
        assert_eq!(
            assets(&WerewolfPresentation::present(&v, &local, &f)),
            vec!["cards/back"]
        );
        assert!(local.selected.is_none());
        assert!(local.reveal.is_none());
    }
}
#[test]
fn role_phase_round_lifecycle_and_seat_replacements_fail_closed_before_input() {
    let v = view(Role::Witch);
    let f = frame(1024.0, 768.0);
    let mut local = WerewolfLocal::default();
    local.set_frame_context(&f);
    click(&mut local, &v, Action::Reveal);
    click(&mut local, &v, Action::Target(SeatId(1)));
    for mut changed in [view(Role::Seer), v.clone(), v.clone(), v.clone(), v.clone()] {
        match &mut changed.perspective {
            Perspective::Seat { seat, .. } => *seat = SeatId(2),
            _ => unreachable!(),
        }
        assert_eq!(
            assets(&WerewolfPresentation::present(&changed, &local, &f)),
            vec!["cards/back"]
        );
    }
    for axis in 0..4 {
        let mut changed = v.clone();
        match axis {
            0 => changed.phase = Phase::Vote,
            1 => changed.round = 2,
            2 => changed.roster[0].status = PlayerStatus::Disconnected,
            _ => {
                changed.perspective = Perspective::Seat {
                    seat: SeatId(0),
                    role: Role::Witch,
                    alive: false,
                    can_act: false,
                }
            }
        }
        let list = WerewolfPresentation::present(&changed, &local, &f);
        assert_eq!(assets(&list), vec!["cards/back"]);
        assert!(!list
            .commands()
            .iter()
            .any(|c| matches!(c,RenderCmd::Text {text,..} if text.contains("Đang chọn:"))));
    }
}
#[test]
fn focus_loss_public_switch_and_all_events_clear_private_local_state() {
    let v = view(Role::Doctor);
    let f = frame(1024.0, 768.0);
    let mut local = WerewolfLocal::default();
    local.set_frame_context(&f);
    click(&mut local, &v, Action::Reveal);
    click(&mut local, &v, Action::Target(SeatId(1)));
    WerewolfPresentation::on_input(&InputEvent::Focus(false), &v, &mut local);
    assert!(local.reveal.is_none() && local.selected.is_none());
    WerewolfPresentation::on_input(&InputEvent::Focus(true), &v, &mut local);
    click(&mut local, &v, Action::Reveal);
    click(&mut local, &v, Action::Public);
    assert_eq!(
        local.take_viewer_request(),
        Some(Viewer::Spectator(SpectatorTier::Live))
    );
    assert_eq!(local.take_viewer_request(), None);
    assert!(local.reveal.is_none());
    click(&mut local, &v, Action::Reveal);
    WerewolfPresentation::on_view_event(
        &ViewEvent::PhaseChanged {
            phase: Phase::Day,
            round: 1,
            ends_at: LogicalTime(40_000),
        },
        &mut local,
        &f,
    );
    assert!(local.reveal.is_none() && local.selected.is_none());
}
#[test]
fn keyboard_reveal_is_explicit_and_selection_is_labelled_before_command() {
    let v = view(Role::Seer);
    let f = frame(1024.0, 768.0);
    let mut local = WerewolfLocal::default();
    local.set_frame_context(&f);
    // Board-first focus order starts at the deliberate own-card reveal.
    for _ in 0..1 {
        WerewolfPresentation::on_input(
            &InputEvent::Key {
                key: Key::Tab,
                pressed: true,
            },
            &v,
            &mut local,
        );
    }
    WerewolfPresentation::on_input(
        &InputEvent::Key {
            key: Key::Enter,
            pressed: true,
        },
        &v,
        &mut local,
    );
    WerewolfPresentation::on_input(
        &InputEvent::Key {
            key: Key::Enter,
            pressed: false,
        },
        &v,
        &mut local,
    );
    assert!(local.is_revealed(&v));
    assert!(click(&mut local, &v, Action::Target(SeatId(1))).is_none());
    let a11y = WerewolfPresentation::a11y(&v, &local);
    assert!(a11y.regions[0]
        .items
        .iter()
        .any(|item| item.state == "Đang chọn mục tiêu"));
    assert_eq!(
        click(&mut local, &v, Action::Submit)
            .unwrap()
            .into_command(),
        Command::Night(NightChoice::Investigate(SeatId(1)))
    );
}
#[test]
fn pointer_commands_roundtrip_through_real_rules_and_rejected_targets_do_not_emit() {
    let roster = SeatRoster::new(
        (0..12)
            .map(|id| SeatEntry {
                seat: SeatId(id),
                occupant: Occupant::Human(UserId(u128::from(id) + 1)),
                team: None,
            })
            .collect(),
    )
    .unwrap();
    let seed = MatchSeed::from_bytes([42; 32]);
    let (mut state, _) = crate::create_initial_state_from_seed(
        &Config::default(),
        &roster,
        LogicalTime::ZERO,
        &seed,
    )
    .unwrap();
    let oracle_seat = *state
        .roles()
        .iter()
        .find(|(_, role)| **role == Role::Seer)
        .unwrap()
        .0;
    let v = WerewolfRules::project(&state, Viewer::Seat(oracle_seat));
    let f = frame(1024.0, 768.0);
    let mut local = WerewolfLocal::default();
    local.set_frame_context(&f);
    assert!(click(&mut local, &v, Action::Target(oracle_seat)).is_none());
    click(&mut local, &v, Action::Reveal);
    assert!(click(&mut local, &v, Action::Target(oracle_seat)).is_none());
    let target = v
        .roster
        .iter()
        .find(|seat| seat.seat != oracle_seat)
        .unwrap()
        .seat;
    click(&mut local, &v, Action::Target(target));
    let command = click(&mut local, &v, Action::Submit)
        .unwrap()
        .into_command();
    let mut rng = tabula_core::DetRng::for_input(&seed, tabula_core::InputIndex(1));
    let mut ctx = Ctx {
        now: LogicalTime(1),
        index: tabula_core::InputIndex(1),
        rng: &mut rng,
        budget: Budget::default(),
    };
    WerewolfRules::apply(
        &mut state,
        Input::Player {
            seat: oracle_seat,
            command,
        },
        &mut ctx,
    )
    .unwrap();
    let after = WerewolfRules::project(&state, Viewer::Seat(oracle_seat));
    assert!(
        matches!(after.knowledge,PrivateKnowledge::Living {choice:Some(NightChoice::Investigate(s)),..} if s==target)
    );
}
#[test]
fn compact_tabs_paging_and_responsive_controls_fit_320_to_1440() {
    for (w, h) in [
        (280.0, 500.0),
        (600.0, 300.0),
        (320.0, 568.0),
        (390.0, 844.0),
        (760.0, 640.0),
        (1024.0, 768.0),
        (1440.0, 900.0),
    ] {
        let f = frame(w, h);
        let mut local = WerewolfLocal::default();
        local.set_frame_context(&f);
        let mut v = view(Role::Witch);
        v.roster.extend((12..20).map(|id| SeatView {
            seat: SeatId(id),
            alive: true,
            status: PlayerStatus::Active,
            role: RoleKnowledge::Hidden,
        }));
        for panel in [Panel::Card, Panel::Table] {
            local.panel = panel;
            local.reveal = reveal_scope(&v);
            for control in controls(&v, &local, f.viewport()) {
                assert!(control.rect.size().x >= 44.0 && control.rect.size().y >= 44.0);
                assert!(control.rect.origin().x >= 0.0 && control.rect.origin().y >= 0.0);
                assert!(
                    control.rect.origin().x + control.rect.size().x <= w + 0.1,
                    "{w}x{h} {:?}",
                    control.action
                );
                assert!(
                    control.rect.origin().y + control.rect.size().y <= h + 0.1,
                    "{w}x{h} {:?}",
                    control.action
                );
            }
            let list = WerewolfPresentation::present(&v, &local, &f);
            assert!(!list.commands().is_empty());
        }
    }
}
#[test]
fn simulator_host_requests_are_one_shot_and_conceal_immediately() {
    let v = view(Role::Werewolf);
    let f = frame(1024.0, 768.0);
    let mut local = WerewolfLocal::default();
    local.set_frame_context(&f);
    click(&mut local, &v, Action::Reveal);
    click(&mut local, &v, Action::NextSeat);
    assert_eq!(local.take_viewer_request(), Some(Viewer::Seat(SeatId(1))));
    assert!(local.reveal.is_none());
    click(&mut local, &v, Action::Advance);
    assert!(local.take_advance_phase_request());
    assert!(!local.take_advance_phase_request());
    click(&mut local, &v, Action::Restart);
    assert!(local.take_restart_request());
    assert!(!local.take_restart_request());
}

#[test]
fn private_submission_report_inventory_and_mark_changes_have_no_concealed_output() {
    let f = frame(1024.0, 768.0);
    let mut local = WerewolfLocal::default();
    local.set_frame_context(&f);
    let original = view(Role::Witch);
    let mut changed = original.clone();
    changed.knowledge = PrivateKnowledge::Living {
        choice: Some(NightChoice::WitchPoison(Some(SeatId(8)))),
        wolf_team: vec![SeatId(2), SeatId(7)],
        seer_reports: BTreeMap::from([(SeatId(4), Alignment::Wolf)]),
        doctor_previous: Some(SeatId(5)),
        hunter_mark: Some(SeatId(9)),
        witch_potions: Some(WitchPotions {
            heal: false,
            poison: false,
        }),
    };
    changed.legal_commands.clear();
    assert_eq!(
        WerewolfPresentation::present(&original, &local, &f).commands(),
        WerewolfPresentation::present(&changed, &local, &f).commands()
    );
    assert_eq!(
        format!("{:?}", WerewolfPresentation::a11y(&original, &local)),
        format!("{:?}", WerewolfPresentation::a11y(&changed, &local))
    );
}

#[test]
fn outside_cannot_reveal_and_votes_work_without_role_reveal() {
    let f = frame(390.0, 844.0);
    let mut local = WerewolfLocal::default();
    local.set_frame_context(&f);
    let mut outside = view(Role::Seer);
    outside.perspective = Perspective::Outside;
    outside.knowledge = PrivateKnowledge::None;
    outside.legal_commands.clear();
    click(&mut local, &outside, Action::Reveal);
    assert!(local.reveal.is_none());
    assert_eq!(
        assets(&WerewolfPresentation::present(&outside, &local, &f)),
        vec!["cards/back"]
    );
    let mut voting = view(Role::Werewolf);
    voting.phase = Phase::Vote;
    voting.legal_commands = vec![
        Command::Vote(Ballot::Target(SeatId(1))),
        Command::Vote(Ballot::Abstain),
        Command::Unvote,
    ];
    local.panel = Panel::Table;
    click(&mut local, &voting, Action::Target(SeatId(1)));
    assert!(local.reveal.is_none());
    assert_eq!(
        click(&mut local, &voting, Action::Submit)
            .unwrap()
            .into_command(),
        Command::Vote(Ballot::Target(SeatId(1)))
    );
    assert_eq!(
        click(&mut local, &voting, Action::Pass)
            .unwrap()
            .into_command(),
        Command::Vote(Ballot::Abstain)
    );
    assert_eq!(
        click(&mut local, &voting, Action::Unvote)
            .unwrap()
            .into_command(),
        Command::Unvote
    );
}

#[test]
fn potion_type_and_own_hunter_mark_are_explicit_authorized_knowledge() {
    let f = frame(1024.0, 768.0);
    let mut local = WerewolfLocal::default();
    local.set_frame_context(&f);
    let mut witch = view(Role::Witch);
    witch
        .legal_commands
        .push(Command::Night(NightChoice::WitchPoison(Some(SeatId(1)))));
    click(&mut local, &witch, Action::Reveal);
    click(&mut local, &witch, Action::Poison);
    click(&mut local, &witch, Action::Target(SeatId(1)));
    assert_eq!(
        click(&mut local, &witch, Action::Submit)
            .unwrap()
            .into_command(),
        Command::Night(NightChoice::WitchPoison(Some(SeatId(1))))
    );
    let mut hunter = view(Role::Hunter);
    hunter.phase = Phase::Day;
    if let PrivateKnowledge::Living { hunter_mark, .. } = &mut hunter.knowledge {
        *hunter_mark = Some(SeatId(9));
    }
    local.conceal();
    click(&mut local, &hunter, Action::Reveal);
    assert!(format!("{:?}", WerewolfPresentation::a11y(&hunter, &local))
        .contains("Mục tiêu đặt trước: Người 10"));
}

#[test]
fn four_semantic_theme_modes_keep_art_and_functional_labels_valid() {
    let v = view(Role::Doctor);
    for kind in [
        tabula_design::ThemeKind::Light,
        tabula_design::ThemeKind::Dark,
        tabula_design::ThemeKind::HighContrastLight,
        tabula_design::ThemeKind::HighContrastDark,
    ] {
        let mut f = frame(390.0, 844.0);
        f = FrameCtx::new(f.viewport(), f.dpi(), 0, Theme::by_kind(kind));
        let mut local = WerewolfLocal::default();
        local.set_frame_context(&f);
        click(&mut local, &v, Action::Reveal);
        let list = WerewolfPresentation::present(&v, &local, &f);
        assert_eq!(assets(&list), vec!["cards/doctor"]);
        assert!(list.commands().iter().any(|command| matches!(command,RenderCmd::Text {text,color,..} if text=="Bác sĩ" && *color==f.theme().game_art.werewolf.card_ink)));
    }
}

#[test]
fn held_activation_cannot_reveal_after_conceal_restart_or_viewer_switch() {
    let v = view(Role::Seer);
    let f = frame(1024.0, 768.0);
    let mut local = WerewolfLocal::default();
    local.set_frame_context(&f);
    local.focus.set_current(Some(FocusId::new(6)));
    WerewolfPresentation::on_input(
        &InputEvent::Key {
            key: Key::Enter,
            pressed: true,
        },
        &v,
        &mut local,
    );
    assert!(local.is_revealed(&v));
    local.conceal();
    WerewolfPresentation::on_input(
        &InputEvent::Key {
            key: Key::Enter,
            pressed: true,
        },
        &v,
        &mut local,
    );
    assert!(!local.is_revealed(&v));
    WerewolfPresentation::on_input(
        &InputEvent::Key {
            key: Key::Enter,
            pressed: false,
        },
        &v,
        &mut local,
    );
    WerewolfPresentation::on_input(
        &InputEvent::Key {
            key: Key::Enter,
            pressed: true,
        },
        &v,
        &mut local,
    );
    assert!(local.is_revealed(&v));
    for key in [Key::Enter, Key::Space] {
        let mut fresh = WerewolfLocal::default();
        fresh.set_frame_context(&f);
        fresh.focus.set_current(Some(FocusId::new(6)));
        fresh.suppress_activation_until_release(key);
        fresh.conceal();
        WerewolfPresentation::on_input(&InputEvent::Key { key, pressed: true }, &v, &mut fresh);
        assert!(!fresh.is_revealed(&v));
        WerewolfPresentation::on_input(
            &InputEvent::Key {
                key,
                pressed: false,
            },
            &v,
            &mut fresh,
        );
        WerewolfPresentation::on_input(&InputEvent::Key { key, pressed: true }, &v, &mut fresh);
        assert!(fresh.is_revealed(&v));
    }
}

#[test]
fn twelve_portraits_and_primary_slot_are_visible_together_in_portrait_and_landscape() {
    // Canvas height already excludes the56dp host row. Geometry is an oracle
    // over the visible viewport, not a screenshot assertion.
    for (width, height) in [
        (390.0, 788.0),
        (320.0, 584.0),
        (844.0, 334.0),
        (1200.0, 824.0),
    ] {
        let f = frame(width, height);
        let v = view(Role::Witch);
        let mut local = WerewolfLocal::default();
        local.set_frame_context(&f);
        let layout = Layout::new(f.viewport());
        let list = controls(&v, &local, f.viewport());
        let targets: Vec<_> = list
            .iter()
            .filter(|c| matches!(c.action, Action::Target(_)))
            .collect();
        assert_eq!(targets.len(), 12);
        for c in targets {
            assert!(layout.table.contains(c.rect.origin()));
            assert!(layout.table.contains(c.rect.origin() + c.rect.size()));
            let d = (c.rect.size().x - 12.0)
                .min(c.rect.size().y - 40.0)
                .clamp(18.0, 64.0);
            assert!(
                d + 38.0 <= c.rect.size().y + 0.1,
                "portrait and both labels overflow {width}x{height}"
            );
        }
        assert!(layout.dock.origin().y + layout.dock.size().y <= layout.footer.origin().y);
        assert!(layout.footer.origin().y + layout.footer.size().y <= height);
        assert_eq!(local.panel, Panel::Table);
    }
}

fn overlaps(a: Rect, b: Rect) -> bool {
    a.origin().x < b.origin().x + b.size().x - 0.1
        && b.origin().x < a.origin().x + a.size().x - 0.1
        && a.origin().y < b.origin().y + b.size().y - 0.1
        && b.origin().y < a.origin().y + a.size().y - 0.1
}

#[test]
fn paging_witch_and_ballot_controls_do_not_overlap_and_modal_actions_are_closed() {
    for (w, h) in [
        (280.0, 500.0),
        (600.0, 300.0),
        (320.0, 584.0),
        (390.0, 788.0),
        (844.0, 334.0),
        (760.0, 640.0),
        (1200.0, 824.0),
    ] {
        let f = frame(w, h);
        let mut local = WerewolfLocal::default();
        local.set_frame_context(&f);
        let mut v = view(Role::Witch);
        v.roster.extend((12..20).map(|id| SeatView {
            seat: SeatId(id),
            alive: true,
            status: PlayerStatus::Active,
            role: RoleKnowledge::Hidden,
        }));
        local.reveal = reveal_scope(&v);
        for phase in [Phase::Night, Phase::Vote] {
            v.phase = phase;
            local.reveal = reveal_scope(&v);
            local.selection_scope = reveal_scope(&v);
            local.selected = Some(SeatId(1));
            if phase == Phase::Vote {
                v.legal_commands = vec![
                    Command::Vote(Ballot::Target(SeatId(1))),
                    Command::Vote(Ballot::Abstain),
                    Command::Unvote,
                ];
            }
            for page in 0..2 {
                local.page = page;
                let cs = controls(&v, &local, f.viewport());
                let enabled: Vec<_> = cs.iter().filter(|c| c.enabled).collect();
                for (i, a) in enabled.iter().enumerate() {
                    for b in enabled.iter().skip(i + 1) {
                        assert!(
                            !overlaps(a.rect, b.rect),
                            "{w}x{h} {:?}/{:?}",
                            a.action,
                            b.action
                        );
                    }
                }
            }
        }
        for panel in [Panel::Table, Panel::Card, Panel::Tools] {
            local.panel = panel;
            let a = WerewolfPresentation::a11y(&v, &local);
            let enabled: Vec<_> = a
                .actions
                .iter()
                .filter(|a| a.enabled)
                .map(|a| &a.id)
                .collect();
            for item in a.regions.iter().flat_map(|r| &r.items) {
                if let Some(action) = &item.activates {
                    assert!(enabled.contains(&action), "dangling action {action:?}");
                }
            }
            if panel == Panel::Tools {
                assert!(a.actions.iter().all(|a| !a.id.0.starts_with("target-")));
                let dialog = Layout::new(f.viewport()).options().dialog;
                let cs = controls(&v, &local, f.viewport());
                for (i, control) in cs.iter().enumerate() {
                    assert!(dialog.contains(control.rect.origin()));
                    assert!(dialog.contains(control.rect.origin() + control.rect.size()));
                    for other in cs.iter().skip(i + 1) {
                        assert!(
                            !overlaps(control.rect, other.rect),
                            "{w}x{h} options overlap"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn reveal_face_swap_cancels_immediately_and_reduced_motion_never_replays_backlog() {
    let f = frame(1024.0, 768.0);
    let v = view(Role::Seer);
    let mut local = WerewolfLocal::default();
    local.set_frame_context(&f);
    local.reveal = reveal_scope(&v);
    local.reveal_started_ms = Some(1000);
    local.deal_cancelled = true;
    let initial = WerewolfPresentation::present(&v, &local, &f);
    assert_eq!(assets(&initial), vec!["cards/back"]);
    let later = FrameCtx::new(f.viewport(), f.dpi(), 1600, f.theme());
    assert_eq!(
        assets(&WerewolfPresentation::present(&v, &local, &later)),
        vec!["cards/seer"]
    );
    WerewolfPresentation::on_input(&InputEvent::Focus(false), &v, &mut local);
    assert_eq!(
        assets(&WerewolfPresentation::present(&v, &local, &later)),
        vec!["cards/back"]
    );
    local.phase_motion = Some((Phase::Dawn, 1000));
    local.vote_motion = Some((SeatId(0), SeatId(1), 1000));
    local.set_reduced_motion(true);
    local.set_reduced_motion(false);
    assert!(
        local.phase_motion.is_none()
            && local.vote_motion.is_none()
            && local.reveal_started_ms.is_none()
    );
    assert!(local.deal_cancelled);
}

#[test]
fn tiny_positive_viewports_are_noninteractive_and_remove_private_output() {
    for (w, h) in [
        (1.0, 1.0),
        (32.0, 120.0),
        (279.0, 844.0),
        (320.0, 480.0),
        (844.0, 299.0),
    ] {
        let f = frame(w, h);
        let mut expected = None;
        for role in [
            Role::Werewolf,
            Role::Seer,
            Role::Doctor,
            Role::Hunter,
            Role::Witch,
            Role::Villager,
        ] {
            let v = view(role);
            let mut local = WerewolfLocal {
                reveal: reveal_scope(&v),
                panel: Panel::Card,
                ..WerewolfLocal::default()
            };
            // Even stale revealed local state cannot reach the tiny canvas.
            let rendered = WerewolfPresentation::present(&v, &local, &f);
            assert!(!rendered
                .commands()
                .iter()
                .any(|c| matches!(c, RenderCmd::Sprite { .. })));
            if let Some(expected) = &expected {
                assert_eq!(&rendered, expected);
            } else {
                expected = Some(rendered);
            }
            local.set_frame_context(&f);
            assert!(local.reveal.is_none());
            assert!(controls(&v, &local, f.viewport()).is_empty());
            let a11y = WerewolfPresentation::a11y(&v, &local);
            assert!(a11y.regions.is_empty() && a11y.actions.is_empty());
            assert!(WerewolfPresentation::on_input(
                &InputEvent::Key {
                    key: Key::Enter,
                    pressed: true
                },
                &v,
                &mut local
            )
            .is_none());
        }
    }
}

#[test]
fn dead_full_vision_lists_authorized_roles_only_after_opening_the_private_region() {
    let f = frame(390.0, 788.0);
    let mut v = view(Role::Seer);
    v.roster[0].alive = false;
    v.roster[1].role = RoleKnowledge::Known(Role::Doctor);
    v.perspective = Perspective::Seat {
        seat: SeatId(0),
        role: Role::Seer,
        alive: false,
        can_act: false,
    };
    v.legal_commands.clear();
    v.knowledge = PrivateKnowledge::Full {
        night_choices: BTreeMap::new(),
        seer_reports: BTreeMap::new(),
        witch_potions: None,
        history: Vec::new(),
        hunter_mark: None,
        hunter_fired: false,
        doctor_previous: None,
    };
    let mut local = WerewolfLocal::default();
    local.set_frame_context(&f);
    let has_other_role = |list: &RenderList| {
        list.commands()
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { text, .. } if text.contains("Người 2: Bác sĩ")))
    };
    assert!(!has_other_role(&WerewolfPresentation::present(
        &v, &local, &f
    )));
    click(&mut local, &v, Action::Reveal);
    assert!(has_other_role(&WerewolfPresentation::present(
        &v, &local, &f
    )));
    assert!(controls(&v, &local, f.viewport())
        .iter()
        .all(|c| !c.enabled || !matches!(c.action, Action::Submit | Action::Target(_))));
    WerewolfPresentation::on_input(&InputEvent::Focus(false), &v, &mut local);
    assert!(!has_other_role(&WerewolfPresentation::present(
        &v, &local, &f
    )));
}

fn fallback_opacities(list: &RenderList) -> Vec<u8> {
    list.commands()
        .iter()
        .filter_map(|command| match command {
            RenderCmd::Rect {
                fill: Some(tabula_presentation::Paint::Solid(color)),
                layer,
                z: 3,
                ..
            } if *layer == tabula_presentation::Layer::HUD => Some(color.alpha()),
            _ => None,
        })
        .collect()
}

#[test]
fn public_deaths_dim_only_the_named_portraits_and_finish_without_frame_backlog() {
    let f = frame(1024.0, 768.0);
    let mut v = view(Role::Seer);
    v.phase = Phase::Dawn;
    v.roster[1].alive = false;
    v.roster[1].role = RoleKnowledge::Known(Role::Doctor);
    let mut local = WerewolfLocal::default();
    local.set_frame_context(&f);
    local.deal_cancelled = true;
    WerewolfPresentation::on_view_event(
        &ViewEvent::DeathRevealed {
            seat: SeatId(1),
            role: Role::Doctor,
        },
        &mut local,
        &f,
    );
    WerewolfPresentation::on_view_event(
        &ViewEvent::PhaseChanged {
            phase: Phase::Dawn,
            round: 1,
            ends_at: LogicalTime(30000),
        },
        &mut local,
        &f,
    );
    let start = WerewolfPresentation::present(&v, &local, &f);
    assert_eq!(fallback_opacities(&start)[2], 255);
    assert!(start
        .commands()
        .iter()
        .any(|c| matches!(c, RenderCmd::Text { text, .. } if text == "Loại")));
    assert!(start
        .commands()
        .iter()
        .any(|c| matches!(c, RenderCmd::Text { text, .. } if text == "Bác sĩ")));
    let mut dense = local.clone();
    let end_ms = f.now_ms() + u64::from(f.theme().motion.exit.duration.milliseconds());
    for now in f.now_ms()..=end_ms {
        dense.set_frame_context(&FrameCtx::new(f.viewport(), f.dpi(), now, f.theme()));
    }
    let end = FrameCtx::new(f.viewport(), f.dpi(), end_ms, f.theme());
    local.set_frame_context(&end);
    let final_list = WerewolfPresentation::present(&v, &local, &end);
    assert_eq!(final_list, WerewolfPresentation::present(&v, &dense, &end));
    assert_eq!(fallback_opacities(&final_list)[2], 114);
    assert_eq!(fallback_opacities(&final_list)[0], 255);
    assert!(local.death_motion.is_empty());
    assert!(assets(&start)
        .iter()
        .all(|asset| !asset.starts_with("cards/doctor")));
}

#[test]
fn public_death_batch_is_bounded_and_cancels_on_blur_reduction_or_unrelated_event() {
    let f = frame(1024.0, 768.0);
    let v = view(Role::Seer);
    let mut local = WerewolfLocal::default();
    local.set_frame_context(&f);
    for id in 0..=20 {
        WerewolfPresentation::on_view_event(
            &ViewEvent::DeathRevealed {
                seat: SeatId(id),
                role: Role::Villager,
            },
            &mut local,
            &f,
        );
    }
    assert_eq!(local.death_motion.len(), 20);
    WerewolfPresentation::on_view_event(
        &ViewEvent::SeerReport {
            seer: SeatId(0),
            target: SeatId(1),
            alignment: Alignment::Wolf,
            round: 1,
        },
        &mut local,
        &f,
    );
    assert!(local.death_motion.is_empty());
    WerewolfPresentation::on_view_event(
        &ViewEvent::DeathRevealed {
            seat: SeatId(1),
            role: Role::Doctor,
        },
        &mut local,
        &f,
    );
    local.set_reduced_motion(true);
    local.set_reduced_motion(false);
    assert!(local.death_motion.is_empty());
    WerewolfPresentation::on_view_event(
        &ViewEvent::DeathRevealed {
            seat: SeatId(1),
            role: Role::Doctor,
        },
        &mut local,
        &f,
    );
    WerewolfPresentation::on_input(&InputEvent::Focus(false), &v, &mut local);
    WerewolfPresentation::on_input(&InputEvent::Focus(true), &v, &mut local);
    assert!(local.death_motion.is_empty());
}

#[test]
fn terminal_outcome_is_immediate_while_win_highlight_is_cancellable() {
    let f = frame(1024.0, 768.0);
    let mut v = view(Role::Seer);
    let outcome = tabula_core::MatchOutcome::new(
        tabula_core::OutcomeKind::Decisive,
        v.roster
            .iter()
            .map(|s| tabula_core::Standing {
                seat: s.seat,
                rank: 0,
                score: 0,
            })
            .collect(),
        "werewolf.village_wins".into(),
    )
    .unwrap();
    v.phase = Phase::Ended;
    v.outcome = Some(outcome.clone());
    v.legal_commands.clear();
    let mut local = WerewolfLocal::default();
    local.set_frame_context(&f);
    WerewolfPresentation::on_view_event(&ViewEvent::MatchEnded { outcome }, &mut local, &f);
    let initial = WerewolfPresentation::present(&v, &local, &f);
    assert!(initial
        .commands()
        .iter()
        .any(|c| matches!(c, RenderCmd::Text { text, .. } if text == "Dân làng chiến thắng")));
    assert!(!can_select(&v, &local));
    let highlight_width = |list: &RenderList| {
        list.commands()
            .iter()
            .find_map(|c| match c {
                RenderCmd::Rect {
                    rect,
                    fill: Some(_),
                    layer,
                    z: 4,
                    ..
                } if *layer == tabula_presentation::Layer::HUD
                    && (rect.size().y - 3.0).abs() < f32::EPSILON =>
                {
                    Some(rect.size().x)
                }
                _ => None,
            })
            .unwrap()
    };
    let end = FrameCtx::new(
        f.viewport(),
        f.dpi(),
        f.now_ms() + u64::from(f.theme().motion.win.duration.milliseconds()),
        f.theme(),
    );
    assert!(
        highlight_width(&initial)
            < highlight_width(&WerewolfPresentation::present(&v, &local, &end))
    );
    local.set_reduced_motion(true);
    assert!(local.win_motion.is_none());
    assert!(
        (highlight_width(&WerewolfPresentation::present(&v, &local, &f))
            - highlight_width(&WerewolfPresentation::present(&v, &local, &end)))
        .abs()
            < f32::EPSILON
    );
    local.set_reduced_motion(false);
    WerewolfPresentation::on_view_event(
        &ViewEvent::MatchEnded {
            outcome: v.outcome.clone().unwrap(),
        },
        &mut local,
        &f,
    );
    WerewolfPresentation::on_input(&InputEvent::Focus(false), &v, &mut local);
    assert!(local.win_motion.is_none());
}

#[test]
fn current_announcement_does_not_repeat_previous_round_deaths_and_counts_current_batch() {
    let mut v = view(Role::Seer);
    let local = WerewolfLocal::default();
    let changed = |phase, round| ViewEvent::PhaseChanged {
        phase,
        round,
        ends_at: LogicalTime(30000),
    };
    v.phase = Phase::Dawn;
    v.round = 2;
    v.public_history = vec![
        changed(Phase::Night, 1),
        ViewEvent::DeathRevealed {
            seat: SeatId(1),
            role: Role::Doctor,
        },
        changed(Phase::Dawn, 1),
        changed(Phase::Night, 2),
        changed(Phase::Dawn, 2),
    ];
    assert_eq!(render::current_death_notice(&v, &local), None);
    v.public_history.insert(
        4,
        ViewEvent::DeathRevealed {
            seat: SeatId(2),
            role: Role::Hunter,
        },
    );
    assert_eq!(
        render::current_death_notice(&v, &local).unwrap(),
        "Người 3 đã bị loại · Thợ săn"
    );
    v.public_history.insert(
        5,
        ViewEvent::DeathRevealed {
            seat: SeatId(3),
            role: Role::Villager,
        },
    );
    assert!(render::current_death_notice(&v, &local)
        .unwrap()
        .starts_with("2 người đã bị loại"));
}

#[test]
fn voting_hint_and_paged_motion_control_have_real_nonoverlapping_slots() {
    let f = frame(320.0, 584.0);
    let mut v = view(Role::Seer);
    v.phase = Phase::Vote;
    v.legal_commands = vec![
        Command::Vote(Ballot::Target(SeatId(1))),
        Command::Vote(Ballot::Abstain),
        Command::Unvote,
    ];
    let mut local = WerewolfLocal::default();
    local.set_frame_context(&f);
    local.selected = Some(SeatId(1));
    local.selection_scope = reveal_scope(&v);
    let (_, hint, style) = render::dock_selection_hint(&v, &local, Layout::new(f.viewport()));
    let unvote = controls(&v, &local, f.viewport())
        .into_iter()
        .find(|c| c.action == Action::Unvote)
        .unwrap();
    assert!(!overlaps(hint, unvote.rect));
    assert_eq!(style, tabula_presentation::TextStyleToken::LabelMd);
    v.roster.extend((12..20).map(|id| SeatView {
        seat: SeatId(id),
        alive: true,
        status: PlayerStatus::Active,
        role: RoleKnowledge::Hidden,
    }));
    local.panel = Panel::Tools;
    let motion = controls(&v, &local, f.viewport())
        .into_iter()
        .find(|c| c.action == Action::Motion)
        .unwrap();
    assert!(motion.enabled && motion.rect.size().x >= 44.0 && motion.rect.size().y >= 44.0);
    assert!(motion.selected, "effects on uses the active button tone");
    click(&mut local, &v, Action::Motion);
    assert!(local.reduced_motion);
    assert!(controls(&v, &local, f.viewport())
        .iter()
        .any(|c| c.action == Action::Motion && !c.selected));
}

#[test]
fn vote_badge_and_managed_image_ring_keep_semantic_contrast_and_stacking() {
    let f = frame(390.0, 788.0);
    let mut v = view(Role::Seer);
    v.phase = Phase::Vote;
    v.votes.insert(SeatId(0), Ballot::Target(SeatId(1)));
    let mut local = WerewolfLocal::default();
    local.set_frame_context(&f);
    let asset = tabula_game_api::AssetRef::new("account/avatar").unwrap();
    let mut displays = tabula_presentation::PublicDisplayMap::new(1);
    displays
        .set(
            SeatId(1),
            tabula_presentation::PublicDisplay::new(
                tabula_presentation::PublicSubject::Guest(99),
                0,
                None,
                Some(asset.clone()),
            ),
        )
        .unwrap();
    let request = displays.avatar_request(SeatId(1)).unwrap();
    assert!(displays.complete_avatar(&request, true));
    local.set_public_display(displays);
    let list = WerewolfPresentation::present(&v, &local, &f);
    assert!(list.commands().iter().any(|c| matches!(c, RenderCmd::Rect { fill: Some(tabula_presentation::Paint::Solid(color)), z: 5, .. } if *color == f.theme().color.primary)));
    assert!(list.commands().iter().any(|c| matches!(c, RenderCmd::Text { text, color, z: 6, .. } if text == "1" && *color == f.theme().color.on_primary)));
    let image = list
        .commands()
        .iter()
        .find_map(|c| match c {
            RenderCmd::Sprite {
                asset: current,
                rect,
                z: 3,
                ..
            } if *current == asset => Some(*rect),
            _ => None,
        })
        .unwrap();
    assert!(list.commands().iter().any(|c| matches!(c, RenderCmd::Rect { rect, fill: None, border: Some(_), z: 4, .. } if *rect == image)));
}
