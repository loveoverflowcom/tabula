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
        0,
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
    WerewolfPresentation::on_input(
        &InputEvent::Pointer {
            position,
            button: PointerButton::Primary,
            phase: PointerPhase::Up,
        },
        view,
        local,
    )
}
fn assets(list: &RenderList) -> Vec<&str> {
    list.commands()
        .iter()
        .filter_map(|c| match c {
            RenderCmd::Sprite { asset, .. } => Some(asset.as_str()),
            _ => None,
        })
        .collect()
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
    // The first three controls select a perspective; the fourth is reveal.
    for _ in 0..4 {
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
