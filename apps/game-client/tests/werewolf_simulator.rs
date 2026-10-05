//! Real generic local-host integration for ADR-0035's isolated-seat simulator.
#![cfg(feature = "werewolf")]
use tabula_core::{
    canonical_encode, MatchSeed, Occupant, SeatEntry, SeatId, SeatRoster, SpectatorTier, UserId,
    Viewer,
};
use tabula_game_api::Input;
use tabula_game_client::LocalMatch;
use tabula_game_werewolf::{
    presentation::WerewolfPresentation, Config, MaxRounds, Phase, PrivateKnowledge, WerewolfRules,
};
use tabula_presentation::{Dpi, FrameCtx, Vec2, Viewport};

type Simulator = LocalMatch<WerewolfRules, WerewolfPresentation>;
fn frame(now: u64) -> FrameCtx {
    FrameCtx::new(
        Viewport::new(Vec2::new(1200.0, 880.0)).unwrap(),
        Dpi::new(1.0).unwrap(),
        now,
        tabula_design::Theme::by_kind(tabula_design::ThemeKind::Dark),
    )
}
fn simulator() -> Simulator {
    let roster = SeatRoster::new(
        (0..12)
            .map(|seat| SeatEntry {
                seat: SeatId(seat),
                occupant: Occupant::Human(UserId(u128::from(seat) + 1)),
                team: None,
            })
            .collect(),
    )
    .unwrap();
    Simulator::new(
        &Config {
            max_rounds: MaxRounds::new(2).unwrap(),
            ..Config::default()
        },
        &roster,
        MatchSeed::from_bytes([19; 32]),
        Viewer::Spectator(SpectatorTier::Live),
    )
    .unwrap()
}
fn switch(game: &mut Simulator, viewer: Viewer) {
    game.local_mut().conceal();
    game.set_viewer(viewer);
}
#[test]
fn private_actions_do_not_change_outsider_projection_or_public_render() {
    let mut game = simulator();
    let f = frame(0);
    let before = canonical_encode(game.view()).unwrap();
    let public = game.present(&f);
    let mut submitted = false;
    for seat in 0..12 {
        switch(&mut game, Viewer::Seat(SeatId(seat)));
        if let Some(command) = game.view().legal_commands.first().copied() {
            game.submit_input(
                Input::Player {
                    seat: SeatId(seat),
                    command,
                },
                &f,
            )
            .unwrap();
            submitted = true;
            break;
        }
    }
    assert!(submitted);
    assert_eq!(game.recorded_inputs().len(), 1);
    switch(&mut game, Viewer::Spectator(SpectatorTier::Live));
    assert_eq!(canonical_encode(game.view()).unwrap(), before);
    assert_eq!(game.present(&f), public);
    assert!(game.view().legal_commands.is_empty());
    assert_eq!(game.view().knowledge, PrivateKnowledge::None);
}
#[test]
fn submitted_night_choices_keep_fixed_deadline_and_host_timer_path() {
    let mut game = simulator();
    let f = frame(0);
    let deadline = game.view().phase_ends_at;
    for seat in 0..12 {
        switch(&mut game, Viewer::Seat(SeatId(seat)));
        if let Some(command) = game.view().legal_commands.first().copied() {
            game.submit_input(
                Input::Player {
                    seat: SeatId(seat),
                    command,
                },
                &f,
            )
            .unwrap();
        }
        assert_eq!(game.view().phase, Phase::Night);
        assert_eq!(game.view().phase_ends_at, deadline);
    }
    switch(&mut game, Viewer::Spectator(SpectatorTier::Live));
    game.advance_to(deadline, &f).unwrap();
    assert_eq!(game.view().phase, Phase::Dawn);
    assert!(matches!(
        game.recorded_inputs().last().unwrap().input,
        Input::Timer { .. }
    ));
    assert_eq!(game.now(), deadline);
}
#[test]
fn timeout_only_complete_matches_are_deterministic_and_reset_is_fresh() {
    fn run() -> (Vec<u8>, usize) {
        let mut game = simulator();
        let f = frame(0);
        while game.ended().is_none() {
            let deadline = game.view().phase_ends_at;
            game.advance_to(deadline, &f).unwrap();
        }
        assert_eq!(game.view().phase, Phase::Ended);
        (
            canonical_encode(game.view()).unwrap(),
            game.replay_trace().accepted_inputs().len(),
        )
    }
    let a = run();
    let b = run();
    assert_eq!(a, b);
    assert_eq!(a.1, 10);
    let fresh = simulator();
    assert_eq!(fresh.view().phase, Phase::Night);
    assert!(fresh.recorded_inputs().is_empty());
    assert!(fresh.ended().is_none());
    assert_eq!(fresh.view().knowledge, PrivateKnowledge::None);
}
#[test]
fn perspective_roundtrip_rebuilds_authorized_knowledge_without_input() {
    let mut game = simulator();
    let public = canonical_encode(game.view()).unwrap();
    for seat in 0..12 {
        switch(&mut game, Viewer::Seat(SeatId(seat)));
        assert!(matches!(
            game.view().knowledge,
            PrivateKnowledge::Living { .. }
        ));
        switch(&mut game, Viewer::Spectator(SpectatorTier::Live));
        assert_eq!(canonical_encode(game.view()).unwrap(), public);
    }
    switch(&mut game, Viewer::Seat(SeatId(250)));
    assert_eq!(canonical_encode(game.view()).unwrap(), public);
    assert!(game.recorded_inputs().is_empty());
}
