//! ADR-0035 opt-in isolated-seat local simulator. No online, auth, social or bot authority.
use macroquad::prelude as mq;
use tabula_core::{
    MatchSeed, Occupant, SeatEntry, SeatId, SeatRoster, SpectatorTier, UserId, Viewer,
};
use tabula_game_api::GameModule;
use tabula_game_client::{
    fixture_assets::{LocalAssetScene, LocalSpriteResources},
    host_resources::{
        acknowledge_concealed_frame, load_builtin_fonts,
        show_asset_failure_with_privacy_ack as show_asset_failure,
    },
    resolve_display_geometry, LocalMatch,
};
use tabula_render_macroquad::MacroquadRenderer;
#[rustfmt::skip]
use tabula_game_werewolf::{ // xtask-allow-game-id: ADR-0035 opt-in local leaf wiring.
    // xtask-allow-game-id: ADR-0035 opt-in local leaf wiring.
    presentation::{assets, WerewolfPresentation},
    Config,
    MaxRounds,
    PhaseDuration,
    PhaseDurations,
    WerewolfModule,
    WerewolfRules,
}; // xtask-allow-game-id: ADR-0035 opt-in local leaf wiring.
use tabula_presentation::{PublicDisplay, PublicDisplayMap, PublicSubject, Renderer};

fn window_conf() -> mq::Conf {
    mq::Conf {
        window_title: "Tabula · Ma Sói · Local simulator".into(),
        window_width: 1200,
        window_height: 880,
        high_dpi: true,
        ..mq::Conf::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let (seats, theme_kind, reduced_motion) = parse_options().await;
    let theme = tabula_design::Theme::by_kind(theme_kind);
    let mut renderer = loop {
        let mut candidate = MacroquadRenderer::new();
        match load_builtin_fonts(&mut candidate).await {
            Ok(()) => break candidate,
            Err(error) => show_asset_failure(&mut candidate, &theme, &error).await,
        }
    };
    let resources = LocalSpriteResources::new(
        assets::MANIFEST,
        WerewolfModule::metadata().id(),
        &assets::asset_pack(),
        vec![],
        assets::gameplay_resources(),
    )
    .expect("closed game-owned declarations");
    #[cfg(not(target_arch = "wasm32"))]
    let resources = resources.with_embedded_images(assets::ALL_IMAGES);
    let mut session = 0u64;
    loop {
        // The asset-cache survives resets; authority/private local state never does.
        let Some((_, dpi)) = resolve_display_geometry(
            mq::screen_width(),
            mq::screen_height(),
            mq::screen_dpi_scale(),
        ) else {
            mq::next_frame().await;
            continue;
        };
        if let Err(error) = resources
            .prepare(&mut renderer, LocalAssetScene::Gameplay, dpi)
            .await
        {
            show_asset_failure(&mut renderer, &theme, &error).await;
            continue;
        }
        let mut game = create_simulator(seats, session);
        game.local_mut().set_reduced_motion(reduced_motion);
        if let Err(error) = run_session(game, &mut renderer, &resources, &theme).await {
            show_asset_failure(&mut renderer, &theme, &error).await;
        }
        let Some(next_session) = session.checked_add(1) else {
            return;
        };
        session = next_session;
    }
}

type Simulator = LocalMatch<WerewolfRules, WerewolfPresentation>;

fn create_simulator(seats: u8, session: u64) -> Simulator {
    let duration = PhaseDuration::from_secs(300).expect("bounded simulation window");
    let short = PhaseDuration::from_secs(2).expect("bounded public announcement window");
    let config = Config {
        phase_durations: PhaseDurations {
            night: duration,
            dawn: short,
            day: duration,
            vote: duration,
            dusk: short,
        },
        max_rounds: MaxRounds::new(10).expect("bounded simulation cap"),
        ..Config::default()
    };
    let roster = SeatRoster::new(
        (0..seats)
            .map(|seat| SeatEntry {
                seat: SeatId(seat),
                occupant: Occupant::Human(UserId(u128::from(seat) + 1)),
                team: None,
            })
            .collect(),
    )
    .expect("bounded distinct local seats");
    // Deterministic local reset scenarios, never passed into a client View.
    let mut seed = [0u8; 32];
    seed[..8].copy_from_slice(&session.to_le_bytes());
    let mut game = LocalMatch::<WerewolfRules, WerewolfPresentation>::new(
        &config,
        &roster,
        MatchSeed::from_bytes(seed),
        Viewer::Spectator(SpectatorTier::Live),
    )
    .expect("validated simulator configuration");
    // This isolated roster uses synthetic human identifiers for referee input;
    // it has no authenticated account/profile source. Public guests therefore
    // share the host's neutral fallback, never a role- or seat-derived image.
    let mut public_display = PublicDisplayMap::new(session);
    for entry in &roster {
        let subject = match entry.occupant {
            Occupant::Human(user) => PublicSubject::Guest(user.0),
            Occupant::Bot { .. } => PublicSubject::Bot(u128::from(entry.seat.0)),
            Occupant::Empty => PublicSubject::Empty,
        };
        public_display
            .set(entry.seat, PublicDisplay::new(subject, 0, None, None))
            .expect("bounded distinct local public display bindings");
    }
    game.local_mut().set_public_display(public_display);
    for (key, native) in [
        (tabula_presentation::Key::Enter, mq::KeyCode::Enter),
        (tabula_presentation::Key::Space, mq::KeyCode::Space),
    ] {
        if mq::is_key_down(native) {
            game.local_mut().suppress_activation_until_release(key);
        }
    }
    game
}

#[derive(Default)]
struct SessionClock {
    start: u64,
    offset: u64,
    ready: bool,
    conceal_pending: bool,
}

async fn run_session(
    mut game: Simulator,
    renderer: &mut MacroquadRenderer,
    resources: &LocalSpriteResources,
    theme: &tabula_design::Theme,
) -> Result<(), String> {
    let mut clock = SessionClock {
        start: clock_ms(),
        ..SessionClock::default()
    };
    let mut prepared_density = None;
    loop {
        let Some((viewport, dpi)) = resolve_display_geometry(
            mq::screen_width(),
            mq::screen_height(),
            mq::screen_dpi_scale(),
        ) else {
            game.local_mut().conceal();
            mq::next_frame().await;
            continue;
        };
        let density = tabula_render_macroquad::density_for_dpi(dpi);
        if prepared_density != Some(density) {
            game.local_mut().conceal();
            resources
                .prepare(renderer, LocalAssetScene::Gameplay, dpi)
                .await?;
            prepared_density = Some(density);
        }
        let elapsed = clock_ms().saturating_sub(clock.start);
        let frame =
            renderer.begin_frame(viewport, dpi, elapsed.saturating_add(clock.offset), *theme);
        game.local_mut().set_frame_context(&frame);
        if let Err(error) = game.advance_frame(&frame) {
            game.local_mut().conceal();
            let _ = renderer.end_frame();
            mq::next_frame().await;
            return Err(format!("Referee stopped: {error:?}"));
        }
        match process_inputs(&mut game, renderer, &frame, &mut clock, elapsed) {
            Ok(true) => {
                let _ = renderer.end_frame();
                mq::next_frame().await;
                return Ok(());
            }
            Err(error) => {
                let _ = renderer.end_frame();
                mq::next_frame().await;
                return Err(error);
            }
            Ok(false) => {}
        }
        game.drain_notices().for_each(drop);
        // Input can contain focus loss and a later activation in one batch.
        // The frame being acknowledged must actually remain opaque, including
        // startup inputs queued before host readiness.
        if !clock.ready || clock.conceal_pending {
            game.local_mut().conceal();
        }
        let visible = tabula_presentation::FrameCtx::new(viewport, dpi, game.now().0, *theme);
        game.local_mut().set_frame_context(&visible);
        let render_ok = renderer.submit(&game.present(&visible)).is_ok();
        let frame_ok = renderer.end_frame().is_ok();
        mq::next_frame().await;
        if !render_ok || !frame_ok {
            game.local_mut().conceal();
            return Err("Runtime render failed; retry starts a fresh local simulation".into());
        }
        if clock.conceal_pending {
            clock.conceal_pending = false;
            acknowledge_concealed_frame().await;
        }
        if !clock.ready {
            // Loading may lose focus before input callbacks are registered. The
            // safe initial public/opaque frame also satisfies any pending shield.
            acknowledge_concealed_frame().await;
            clock.ready = true;
            notify_ready().await;
        }
    }
}

fn process_inputs(
    game: &mut Simulator,
    renderer: &mut MacroquadRenderer,
    frame: &tabula_presentation::FrameCtx,
    clock: &mut SessionClock,
    elapsed: u64,
) -> Result<bool, String> {
    for event in renderer.drain_input() {
        if matches!(event, tabula_presentation::InputEvent::Focus(false)) {
            clock.conceal_pending = true;
        }
        if let Err(error) = game.handle_presentation_input(&event, frame) {
            game.local_mut().conceal();
            macroquad::logging::warn!("Local action rejected: {error:?}");
        }
        if let Some(viewer) = game.local_mut().take_viewer_request() {
            game.local_mut().conceal();
            game.set_viewer(viewer);
        }
        if game.local_mut().take_restart_request() {
            return Ok(true);
        }
        if game.local_mut().take_advance_phase_request() && game.ended().is_none() {
            game.local_mut().conceal();
            let deadline = game.view().phase_ends_at;
            // The simulator jumps logical time; projected feedback starts on
            // that same clock rather than appearing hundreds of seconds stale.
            let transition_frame = tabula_presentation::FrameCtx::new(
                frame.viewport(),
                frame.dpi(),
                deadline.0,
                frame.theme(),
            );
            game.advance_to(deadline, &transition_frame)
                .map_err(|error| format!("Referee stopped: {error:?}"))?;
            clock.offset = deadline.0.saturating_sub(elapsed);
        }
    }
    Ok(false)
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::float_arithmetic
)]
fn clock_ms() -> u64 {
    let value = mq::get_time() * 1000.0;
    if value.is_finite() && value > 0.0 {
        value as u64
    } else {
        0
    }
}
#[cfg_attr(not(target_arch = "wasm32"), allow(clippy::unused_async))]
async fn notify_ready() {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = mq::load_file("tabula-ready.txt").await;
    }
}
#[cfg_attr(not(target_arch = "wasm32"), allow(clippy::unused_async))]
async fn parse_options() -> (u8, tabula_design::ThemeKind, bool) {
    #[cfg(not(target_arch = "wasm32"))]
    let args: Vec<String> = std::env::args().skip(1).collect();
    #[cfg(target_arch = "wasm32")]
    let args: Vec<String> = mq::load_file("tabula-launch.txt")
        .await
        .ok()
        .filter(|bytes| bytes.len() <= 4096)
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .map(|text| text.lines().take(64).map(str::to_owned).collect())
        .unwrap_or_default();
    let seats = args
        .windows(2)
        .find(|pair| pair[0] == "--seats")
        .and_then(|pair| pair[1].parse::<u8>().ok())
        .filter(|seats| (6..=20).contains(seats))
        .unwrap_or(12);
    let theme = args
        .windows(2)
        .find(|pair| pair[0] == "--theme")
        .and_then(|pair| tabula_game_client::runtime_ui::parse_local_theme(&pair[1]))
        .unwrap_or(tabula_design::ThemeKind::Dark);
    (
        seats,
        theme,
        args.iter().any(|arg| arg == "--reduced-motion"),
    )
}
