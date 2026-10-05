//! Macroquad platform entry point for local hot-seat gameplay wiring.
//!
//! # One generic loop, multiple games
//!
//! [`run_local`] is generic over `(GameRules, GamePresentation)` and contains no
//! game-specific behaviour: it resolves the viewport, advances logical time,
//! feeds normalized input through the presenter, optionally drives bot seats,
//! and submits the render list. Everything a game contributes is passed in —
//! its config, its roster, and two closures that name facts only the game can
//! know (which seat is on turn, and whether the match wants a bot to move).
//!
//! The `SelectedGame` match below is the only place a game is named, and that
//! is the Phase-2 local vertical slice's deliberate leaf wiring: Phase 4
//! replaces it with `tabula-registry` (doc 01 §5.1).

mod clock_options;
mod standalone_setup;
#[cfg(all(feature = "online", target_arch = "wasm32"))]
mod online_runtime;

use clock_options::{LocalClockControl, LocalClockOptions};
use macroquad::prelude as mq;
#[cfg(feature = "tiles")] // xtask-allow-game-id: optional Phase 3 local vertical slice wiring.
use tabula_core::BotLevel;
use tabula_core::{
    DetRng, InputIndex, MatchSeed, Millis, Occupant, SeatEntry, SeatId, SeatRoster, UserId, Viewer,
};
use tabula_game_api::{GameBot, GameModule, GameRules};
#[rustfmt::skip]
use tabula_game_chess::{ // xtask-allow-game-id: direct Phase 2 local vertical slice wiring.
    presentation::ChessPresentation, ChessRules, ClockConfig, ClockControl, Config as ChessConfig,
};
use tabula_game_client::{
    fixture_assets::{LocalAssetScene, LocalSpriteResources},
    resolve_display_geometry,
    runtime_ui::{parse_local_theme, FeedbackInput, LocalFeedback},
    LocalMatch,
};
#[rustfmt::skip]
#[cfg(feature = "tiles")] // xtask-allow-game-id: optional Phase 3 local vertical slice wiring.
use tabula_game_tiles::{ // xtask-allow-game-id: direct Phase 3 local vertical slice wiring.
    presentation::{fixture, TilesPresentation},
    rules::{MAX_SEATS as MAX_PLACEMENT_SEATS, MIN_SEATS as MIN_PLACEMENT_SEATS},
    Config as TilesConfig, TilesModule, TilesRules,
};
use tabula_presentation::{AudioSink, GamePresentation, Renderer};
use tabula_render_macroquad::{MacroquadAudioSink, MacroquadRenderer};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum SelectedGame {
    #[default]
    Chess, // xtask-allow-game-id: direct Phase 2 local vertical slice wiring.
    #[cfg(feature = "tiles")] // xtask-allow-game-id: optional Phase 3 local vertical slice wiring.
    Tiles, // xtask-allow-game-id: direct Phase 3 local vertical slice wiring.
}

/// How the local shell fills the seats nobody is sitting at.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum SeatFill {
    /// Every seat is played by whoever is at the keyboard; the viewer follows
    /// the seat on turn.
    #[default]
    HotSeat,
    /// Seat 0 is human, every other seat is driven by the game's own bot.
    Solo,
}

fn window_conf() -> mq::Conf {
    mq::Conf {
        window_title: String::from("Tabula — local game"),
        window_width: 900,
        window_height: 720,
        high_dpi: true,
        ..mq::Conf::default()
    }
}

#[macroquad::main(window_conf)]
#[rustfmt::skip]
async fn main() {
    let mut audio = MacroquadAudioSink::new();
    let options = parse_options().await;
    let theme = tabula_design::Theme::by_kind(options.theme);
    let mut renderer = loop {
        let mut candidate = MacroquadRenderer::new();
        match load_builtin_fonts(&mut candidate).await {
            Ok(()) => break candidate,
            Err(error) => {
                macroquad::logging::error!("{error}");
                show_asset_failure(&mut candidate, &theme, &error).await;
                // Font replacement after drawing is forbidden by the renderer.
                // Retry on a fresh renderer before any match has been created.
            }
        }
    };

    // The only place a game is named. Each arm is a one-liner so rustfmt keeps
    // its trailing comment, which is what lets it carry its own I-9
    // suppression marker instead of the whole block sharing one.
    loop {
        match options.game {
            SelectedGame::Chess => run_chess(&mut renderer, &mut audio, &theme, options.clone()).await, // xtask-allow-game-id: direct Phase 2 local vertical slice wiring.
            #[cfg(feature = "tiles")] // xtask-allow-game-id: optional Phase 3 local vertical slice wiring.
            SelectedGame::Tiles => run_tiles(&mut renderer, &mut audio, &theme, options.clone()).await, // xtask-allow-game-id: direct Phase 3 local vertical slice wiring.
        }
        // A completed or stopped session returns only after New local game.
        // activation. Reconstruct the match, local state, clocks and bot RNG.
    }
}

/// Brand fonts use bounded external aliases on WASM and embedded bytes on native.
#[cfg_attr(not(target_arch = "wasm32"), allow(clippy::unused_async))]
async fn load_builtin_fonts(renderer: &mut MacroquadRenderer) -> Result<(), String> {
    #[cfg(not(target_arch = "wasm32"))]
    let (text, strong, display) = (
        include_bytes!("../../../assets/fonts/OpenSans-Regular.ttf").as_slice(),
        include_bytes!("../../../assets/fonts/OpenSans-Semibold.ttf").as_slice(),
        include_bytes!("../../../assets/fonts/NotoSerif-Bold.ttf").as_slice(),
    );
    #[cfg(target_arch = "wasm32")]
    let (text, strong, display) = {
        let mut fonts = Vec::with_capacity(3);
        for path in [
            "assets/OpenSans-Regular.ttf",
            "assets/OpenSans-Semibold.ttf",
            "assets/NotoSerif-Bold.ttf",
        ] {
            let bytes = mq::load_file(path)
                .await
                .map_err(|error| format!("font load {path}: {error}"))?;
            if bytes.is_empty() || bytes.len() > 256 * 1024 {
                return Err(format!("font load {path}: invalid bounded size"));
            }
            fonts.push(bytes);
        }
        let mut fonts = fonts.into_iter();
        (
            fonts.next().expect("three declared fonts"),
            fonts.next().expect("three declared fonts"),
            fonts.next().expect("three declared fonts"),
        )
    };
    #[cfg(target_arch = "wasm32")]
    let (text, strong, display) = (text.as_slice(), strong.as_slice(), display.as_slice());
    renderer
        .set_builtin_font_bytes(text, strong, display)
        .map_err(|error| format!("built-in fonts: {error:?}"))
}

#[rustfmt::skip]
async fn run_chess( // xtask-allow-game-id: direct Phase 2 local vertical slice wiring.
    renderer: &mut MacroquadRenderer,
    audio: &mut MacroquadAudioSink,
    theme: &tabula_design::Theme,
    mut options: Options,
) {
    let resources = LocalSpriteResources::new(
        tabula_game_chess::presentation::assets::MANIFEST, // xtask-allow-game-id: direct Phase 2 local vertical slice wiring.
        tabula_game_chess::ChessModule::metadata().id(), // xtask-allow-game-id: direct Phase 2 local vertical slice wiring.
        &ChessPresentation::asset_pack(),
        tabula_game_chess::presentation::assets::setup_resources(), // xtask-allow-game-id: direct Phase 2 local vertical slice wiring.
        tabula_game_chess::presentation::assets::gameplay_resources(), // xtask-allow-game-id: direct Phase 2 local vertical slice wiring.
    );
    let resources = match resources {
        Ok(resources) => resources,
        Err(error) => {
            show_asset_failure(renderer, theme, &error).await;
            return;
        }
    };
    #[cfg(not(target_arch = "wasm32"))]
    let resources = resources.with_embedded_images(
        tabula_game_chess::presentation::assets::ALL_IMAGES, // xtask-allow-game-id: direct Phase 2 local vertical slice wiring.
    );
    if !options.skip_setup {
        match run_setup(renderer, theme, options.clock, &resources).await {
            Ok(clock) => options.clock = clock,
            Err(error) => {
                show_asset_failure(renderer, theme, &error).await;
                return;
            }
        }
    }
    // Initial I/O completes before constructing the match and starting its clock.
    if let Err(error) = resources.prepare(renderer, LocalAssetScene::Gameplay, display_dpi().await).await {
        show_asset_failure(renderer, theme, &error).await;
        return;
    }
    if let Some(match_id) = options.online_match.as_deref() {
        #[cfg(all(feature = "online", target_arch = "wasm32"))]
        online_runtime::run_online::<tabula_game_chess::ChessModule, ChessPresentation>(renderer, audio, theme, match_id, options.reduced_motion, &resources).await; // xtask-allow-game-id: existing typed leaf presenter wiring; online loop is generic.
        #[cfg(not(all(feature = "online", target_arch = "wasm32")))]
        { let _ = match_id; show_asset_failure(renderer, theme, "Online play is unavailable in this build").await; }
        return;
    }
    let mut local_match = LocalMatch::<ChessRules, ChessPresentation>::new(
        &ChessConfig {
            clock: configured_clock(options.clock),
        },
        &human_roster(2),
        MatchSeed::from_bytes([0; 32]),
        Viewer::Seat(SeatId(0)),
    )
    .expect("the bounded local configuration is valid");
    local_match.local_mut().set_hot_seat_controls(true);
    run_local(
        local_match,
        renderer,
        audio,
        theme,
        |view| view.turn.seat(),
        None,
        &[],
        options.reduced_motion,
        Some(&resources),
    )
    .await;
}

fn configured_clock(options: LocalClockOptions) -> Option<ClockConfig> {
    match options.control {
        LocalClockControl::Untimed => None,
        control => Some(ClockConfig {
            initial: Millis(options.initial_ms),
            control: match control {
                LocalClockControl::Bronstein => ClockControl::Bronstein {
                    delay: Millis(options.adjustment_ms),
                },
                LocalClockControl::Fischer | LocalClockControl::Untimed => ClockControl::Fischer {
                    increment: Millis(options.adjustment_ms),
                },
            },
        }),
    }
}

/// The native entry owns setup only; constructing a match starts its clock.
async fn run_setup(
    renderer: &mut MacroquadRenderer,
    theme: &tabula_design::Theme,
    clock: LocalClockOptions,
    resources: &LocalSpriteResources,
) -> Result<LocalClockOptions, String> {
    let mut setup = standalone_setup::StandaloneSetup::new(clock);
    let mut prepared_density = None;
    for (key, native) in [
        (tabula_presentation::Key::Enter, mq::KeyCode::Enter),
        (tabula_presentation::Key::Space, mq::KeyCode::Space),
    ] {
        if mq::is_key_down(native) {
            setup.suppress_held_key(key);
        }
    }
    loop {
        let Some((viewport, dpi)) = resolve_display_geometry(
            mq::screen_width(),
            mq::screen_height(),
            mq::screen_dpi_scale(),
        ) else {
            mq::next_frame().await;
            continue;
        };
        let density = tabula_render_macroquad::density_for_dpi(dpi);
        if prepared_density != Some(density) {
            resources
                .prepare(renderer, LocalAssetScene::Setup, dpi)
                .await?;
            prepared_density = Some(density);
        }
        let frame = renderer.begin_frame(viewport, dpi, 0, *theme);
        for event in renderer.drain_input() {
            if let Some(clock) = setup.on_input(&event, &frame) {
                let _ = renderer.end_frame();
                mq::next_frame().await;
                return Ok(clock);
            }
        }
        let cover = tabula_game_chess::presentation::assets::cover_asset(); // xtask-allow-game-id: direct Phase 2 local vertical slice wiring.
        if let Ok(scene) = setup.present(&frame, cover) {
            let _ = renderer.submit(&scene);
        }
        let _ = renderer.end_frame();
        mq::next_frame().await;
    }
}

/// Waits for valid geometry without choosing an invented initial density.
async fn display_dpi() -> tabula_presentation::Dpi {
    loop {
        if let Some((_, dpi)) = resolve_display_geometry(
            mq::screen_width(),
            mq::screen_height(),
            mq::screen_dpi_scale(),
        ) {
            return dpi;
        }
        mq::next_frame().await;
    }
}

/// Asset failure never produces a false ready game or an invisible board.
#[allow(clippy::float_arithmetic)] // Screen-space recovery geometry never enters canonical state.
async fn show_asset_failure(
    renderer: &mut MacroquadRenderer,
    theme: &tabula_design::Theme,
    error: &str,
) {
    use tabula_presentation::{
        ActionButton, Align, ButtonInteraction, ButtonTone, Camera2D, FocusGraph, FocusId,
        FocusNode, FocusState, Layer, NavigationAction, Rect, RenderCmd, RenderListBuilder,
        TextStyleToken,
    };
    let mut interaction = ButtonInteraction::default();
    let mut focus = FocusState::default();
    let id = FocusId::new(0);
    focus.set_keyboard_focus(Some(id));
    for (key, native) in [
        (tabula_presentation::Key::Enter, mq::KeyCode::Enter),
        (tabula_presentation::Key::Space, mq::KeyCode::Space),
    ] {
        if mq::is_key_down(native) {
            interaction.suppress_activation_until_release(key);
        }
    }
    loop {
        let Some((viewport, dpi)) = resolve_display_geometry(
            mq::screen_width(),
            mq::screen_height(),
            mq::screen_dpi_scale(),
        ) else {
            mq::next_frame().await;
            continue;
        };
        let _frame = renderer.begin_frame(viewport, dpi, 0, *theme);
        let size = viewport.size();
        let rect = Rect::new(
            glam::Vec2::new(24.0, 220.0),
            glam::Vec2::new((size.x - 48.0).max(44.0), 48.0),
        )
        .expect("bounded recovery target");
        let button = ActionButton::new(id, rect, "Retry assets", theme.density.min_target)
            .expect("44dp recovery")
            .tone(ButtonTone::Filled);
        let graph =
            FocusGraph::new(vec![FocusNode::new(id, rect)]).expect("single recovery action");
        for event in renderer.drain_input() {
            if matches!(
                interaction.on_input(&event, &[button], &graph, &mut focus),
                NavigationAction::Activate(_)
            ) {
                let _ = renderer.end_frame();
                mq::next_frame().await;
                return;
            }
        }
        let mut builder = RenderListBuilder::new(Camera2D::default());
        for (text, y, style) in [
            (
                "Local resources could not load",
                48.0,
                TextStyleToken::HeadlineMd,
            ),
            (error, 110.0, TextStyleToken::BodyMd),
        ] {
            let _ = builder.push(RenderCmd::Text {
                text: text.to_owned(),
                at: glam::Vec2::new(24.0, y),
                style,
                align: Align::Start,
                max_width: tabula_design::Positive::new((size.x - 48.0).max(1.0)).ok(),
                color: theme.color.on_surface,
                layer: Layer::HUD,
                z: 0,
            });
        }
        let _ = button.draw(&mut builder, theme, &interaction, &focus, Layer::HUD);
        if let Ok(scene) = builder.finish() {
            let _ = renderer.submit(&scene);
        }
        let _ = renderer.end_frame();
        mq::next_frame().await;
    }
}

#[rustfmt::skip]
#[cfg(feature = "tiles")] // xtask-allow-game-id: optional Phase 3 local vertical slice wiring.
async fn run_tiles( // xtask-allow-game-id: direct Phase 3 local vertical slice wiring.
    renderer: &mut MacroquadRenderer,
    audio: &mut MacroquadAudioSink,
    theme: &tabula_design::Theme,
    options: Options,
) {
    if options.online_match.is_some() {
        show_asset_failure(renderer, theme, "This selected game has no direct online host").await;
        return;
    }
    if let Err(error) = tabula_game_client::fixture_assets::preload_sprite_fixture(
        renderer,
        fixture::MANIFEST,
        TilesModule::metadata().id(),
        &TilesPresentation::asset_pack(),
        &[
            (tabula_assets::AssetDensity::new(1).expect("valid fixture density"), fixture::ATLAS_1X),
            (tabula_assets::AssetDensity::new(2).expect("valid fixture density"), fixture::ATLAS_2X),
        ],
    ).await {
        // A missing/failed texture remains an explicit preflight failure, and
        // the ordinary screen-space recovery dock is still available.
        macroquad::logging::error!("{error}");
    }
    let seats = options
        .seats
        .clamp(MIN_PLACEMENT_SEATS, MAX_PLACEMENT_SEATS);
    let local_match = LocalMatch::<TilesRules, TilesPresentation>::new(
        &TilesConfig {
            // Local play has no deadline: the human takes as long as they
            // like, and the async path is the same rules with a nonzero value
            // here (docs/games/tiles.md).
            turn_deadline_ms: 0,
        },
        &human_roster(seats),
        MatchSeed::from_bytes([0; 32]),
        Viewer::Seat(SeatId(0)),
    )
    .expect("the fixed local configuration is valid");
    let bot_seats: Vec<SeatId> = match options.fill {
        SeatFill::HotSeat => Vec::new(),
        SeatFill::Solo => (1..seats).map(SeatId).collect(),
    };
    run_local(
        local_match,
        renderer,
        audio,
        theme,
        |view| view.turn,
        TilesModule::bot(BotLevel::Easy),
        &bot_seats,
        options.reduced_motion,
        None,
    )
    .await;
}

/// The whole local gameplay loop, with no game-specific branch in it.
///
/// `turn_of` is the one fact a generic loop cannot derive: which seat's
/// projection to show in hot seat. `bot`/`bot_seats` let the shell drive the
/// seats nobody is sitting at — a bot is a seat whose commands come from a
/// function of that seat's projection (doc 00 §6.5), so its answer goes in
/// through the ordinary player path and can be rejected like anyone else's.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)] // Keep frame/input/effect ordering explicit in the one local loop.
async fn run_local<R, P>(
    mut local_match: LocalMatch<R, P>,
    renderer: &mut MacroquadRenderer,
    audio: &mut MacroquadAudioSink,
    theme: &tabula_design::Theme,
    turn_of: fn(&R::View) -> SeatId,
    bot: Option<Box<dyn GameBot<R>>>,
    bot_seats: &[SeatId],
    reduced_motion: bool,
    resources: Option<&LocalSpriteResources>,
) where
    R: GameRules,
    P: GamePresentation<Rules = R>,
    P::Local: SetViewport,
{
    // The bot's randomness is derived from the same deterministic kernel the
    // rules use, so a local session with bots is as reproducible as one
    // without.
    let mut bot_rng = DetRng::for_input(&MatchSeed::from_bytes([0; 32]), InputIndex(u64::MAX));
    let mut feedback = fresh_feedback();
    local_match.local_mut().set_reduced_motion(reduced_motion);
    let started_at_ms = presentation_now_ms();
    let mut ready_notified = false;
    let mut prepared_density = None;

    'game_loop: loop {
        let Some((viewport, dpi)) = resolve_display_geometry(
            mq::screen_width(),
            mq::screen_height(),
            mq::screen_dpi_scale(),
        ) else {
            // A transient zero or non-finite viewport/DPI (startup,
            // backgrounding, rapid browser resize) is skipped at the shell
            // boundary rather than weakening the validated `Viewport` type.
            mq::next_frame().await;
            continue 'game_loop;
        };
        // The prior next_frame has flushed submitted texture leases. A changed
        // DPI may now fetch/decode its selected variant before begin_frame,
        // never during synchronous rendering or after claiming readiness.
        let density = tabula_render_macroquad::density_for_dpi(dpi);
        if let Some(resources) = resources.filter(|_| prepared_density != Some(density)) {
            if let Err(error) = resources
                .prepare(renderer, LocalAssetScene::Gameplay, dpi)
                .await
            {
                show_asset_failure(renderer, theme, &error).await;
                return;
            }
            prepared_density = Some(density);
        }
        let frame = renderer.begin_frame(
            viewport,
            dpi,
            session_elapsed_ms(started_at_ms, presentation_now_ms()),
            *theme,
        );
        let board_frame = sync_local_frame(&mut local_match, &mut feedback, &frame);

        if feedback.allows_gameplay(local_match.ended().is_some()) {
            match local_match.advance_frame(&board_frame) {
                Ok(cues) => play_cues(audio, &cues),
                Err(error) => feedback.note_timer_error(&error),
            }
        }
        sync_local_frame(&mut local_match, &mut feedback, &frame);

        for event in renderer.drain_input() {
            if bot_seats.is_empty() {
                let override_viewer = local_match.local_mut().viewer_override();
                local_match.set_viewer(
                    override_viewer.unwrap_or_else(|| Viewer::Seat(turn_of(local_match.view()))),
                );
            }
            match feedback.route_input(&mut local_match, &event, &frame) {
                FeedbackInput::NewLocalGame => {
                    renderer.end_frame().expect("an active frame can be ended");
                    mq::next_frame().await;
                    return;
                }
                FeedbackInput::Consumed | FeedbackInput::CancelGameplayPointer => continue,
                FeedbackInput::PassThrough => {}
            }
            // Fatal feedback consumed input above. Terminal matches still allow
            // local inspection; LocalMatch discards any emitted intent.
            let accepted_before = local_match.replay_trace().accepted_inputs().len();
            let was_ended = local_match.ended().is_some();
            let board_frame = sync_local_frame(&mut local_match, &mut feedback, &frame);
            match local_match.handle_presentation_input(&event, &board_frame) {
                Ok(cues) => {
                    play_cues(audio, &cues);
                    if local_match.replay_trace().accepted_inputs().len() > accepted_before {
                        feedback.note_accepted_input();
                    }
                }
                Err(error) => {
                    feedback.note_error(&error);
                    feedback.suppress_opening_input(&event);
                }
            }
            sync_local_frame(&mut local_match, &mut feedback, &frame);
            if !was_ended && local_match.ended().is_some() {
                feedback.suppress_opening_input(&event);
            }
        }

        // Bot seats. Requests the rules raise explicitly are drained first so
        // a game that emits `RequestBotMove` is not second-guessed.
        let requested: Vec<SeatId> = local_match
            .drain_bot_requests()
            .map(|request| request.seat)
            .collect();
        if let Some(bot) = bot
            .as_ref()
            .filter(|_| feedback.allows_gameplay(local_match.ended().is_some()))
        {
            let on_turn = turn_of(local_match.view());
            let due = requested
                .iter()
                .copied()
                .chain(bot_seats.iter().copied().filter(|seat| *seat == on_turn));
            for seat in due {
                if !feedback.allows_gameplay(local_match.ended().is_some()) {
                    break;
                }
                local_match.set_viewer(Viewer::Seat(seat));
                let Some(command) = bot.choose(local_match.view(), seat, &mut bot_rng) else {
                    continue;
                };
                match local_match.submit_bot_move(seat, command, &frame) {
                    Ok(cues) => play_cues(audio, &cues),
                    Err(error) => feedback.note_error(&error),
                }
            }
        }

        // Notices carry an explicit audience. Until a viewer-filtered notice
        // adapter exists, discard them without leaking payloads into logs/UI.
        local_match.drain_notices().for_each(drop);

        // Hot seat: show whoever is on turn. With bots, the human at seat 0
        // keeps their own view.
        let viewer = if bot_seats.is_empty() {
            local_match
                .local_mut()
                .viewer_override()
                .unwrap_or_else(|| Viewer::Seat(turn_of(local_match.view())))
        } else {
            Viewer::Seat(SeatId(0))
        };
        local_match.set_viewer(viewer);

        let board_frame = sync_local_frame(&mut local_match, &mut feedback, &frame);
        present_local_scenes(&local_match, &mut feedback, renderer, &frame, &board_frame);
        let frame_ok = renderer.end_frame().is_ok();
        if !frame_ok {
            feedback.note_render_error();
        }
        mq::next_frame().await;
        if frame_ok && feedback.allows_gameplay(false) && !ready_notified {
            // Readiness follows a real submitted gameplay frame and its flush,
            // not an async preloading frame or merely a fetched WASM binary.
            ready_notified = true;
            notify_runtime_ready().await;
        }
    }
}

#[cfg_attr(not(target_arch = "wasm32"), allow(clippy::unused_async))]
async fn notify_runtime_ready() {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = mq::load_file("tabula-ready.txt").await;
    }
}

/// Draws the board and screen-space feedback, preserving fatal render recovery.
fn present_local_scenes<R, P>(
    local_match: &LocalMatch<R, P>,
    feedback: &mut LocalFeedback,
    renderer: &mut MacroquadRenderer,
    frame: &tabula_presentation::FrameCtx,
    board_frame: &tabula_presentation::FrameCtx,
) where
    R: GameRules,
    P: GamePresentation<Rules = R>,
{
    if feedback.allows_gameplay(false)
        && renderer.submit(&local_match.present(board_frame)).is_err()
    {
        feedback.note_render_error();
    }
    // Supported screen-space primitives remain available even when the board
    // failed preflight. The dock uses the full frame, never the board camera.
    if let Ok(scene) = feedback.present(frame) {
        let _ = renderer.submit(&scene);
    }
}

/// Uses one viewport for board input/rendering and suppresses held completion keys.
fn sync_local_frame<R, P>(
    local_match: &mut LocalMatch<R, P>,
    feedback: &mut LocalFeedback,
    frame: &tabula_presentation::FrameCtx,
) -> tabula_presentation::FrameCtx
where
    R: GameRules,
    P: GamePresentation<Rules = R>,
    P::Local: SetViewport,
{
    if feedback.sync_match(local_match) {
        suppress_held_activation(feedback);
    }
    let board_frame = feedback.board_frame(frame);
    local_match.local_mut().sync_frame(&board_frame);
    board_frame
}

/// The one thing the generic loop needs from a presenter's local state.
///
/// Every game already has this method; naming it as a trait is what lets one
/// loop serve all of them instead of one loop per game.
trait SetViewport {
    fn sync_frame(&mut self, frame: &tabula_presentation::FrameCtx);
    fn set_reduced_motion(&mut self, _reduced: bool) {}
    fn viewer_override(&self) -> Option<Viewer> {
        None
    }
}

#[rustfmt::skip]
impl SetViewport for tabula_game_chess::presentation::ChessLocal { // xtask-allow-game-id: direct Phase 2 local vertical slice wiring.
    fn sync_frame(&mut self, frame: &tabula_presentation::FrameCtx) {
        Self::set_viewport(self, frame.viewport());
    }
    fn set_reduced_motion(&mut self, reduced: bool) { Self::set_reduced_motion(self, reduced); }
    fn viewer_override(&self) -> Option<Viewer> { Self::viewer_override(self) }
}

#[rustfmt::skip]
#[cfg(feature = "tiles")] // xtask-allow-game-id: optional Phase 3 local vertical slice wiring.
impl SetViewport for tabula_game_tiles::presentation::TilesLocal { // xtask-allow-game-id: direct Phase 3 local vertical slice wiring.
    fn sync_frame(&mut self, frame: &tabula_presentation::FrameCtx) {
        Self::set_frame_context(self, frame);
    }
    fn set_reduced_motion(&mut self, reduced: bool) {
        Self::set_reduced_motion(self, reduced);
    }
}

#[derive(Clone, Debug)]
struct Options {
    game: SelectedGame,
    seats: u8,
    fill: SeatFill,
    theme: tabula_design::ThemeKind,
    reduced_motion: bool,
    clock: LocalClockOptions,
    skip_setup: bool,
    online_match: Option<String>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            game: SelectedGame::default(),
            seats: 3,
            fill: SeatFill::default(),
            theme: tabula_design::ThemeKind::Light,
            reduced_motion: false,
            clock: LocalClockOptions::default(),
            skip_setup: false,
            online_match: None,
        }
    }
}

#[cfg_attr(not(target_arch = "wasm32"), allow(clippy::unused_async))]
async fn parse_options() -> Options {
    #[cfg(not(target_arch = "wasm32"))]
    {
        parse_options_from(std::env::args().skip(1))
    }
    #[cfg(target_arch = "wasm32")]
    {
        // The checked-in host exposes exactly this bounded virtual file through
        // Macroquad's ordinary safe file API. It contains configuration only.
        match mq::load_file("tabula-launch.txt").await {
            Ok(bytes) if bytes.len() <= 4096 => match String::from_utf8(bytes) {
                Ok(value) if value.lines().count() <= 64 => {
                    parse_options_from(value.lines().map(str::to_owned))
                }
                _ => Options::default(),
            },
            _ => Options::default(),
        }
    }
}

fn parse_options_from(mut args: impl Iterator<Item = String>) -> Options {
    let mut options = Options::default();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--game" => {
                if let Some(name) = args.next() {
                    options.game = parse_game(&name);
                }
            }
            "--seats" => {
                if let Some(value) = args.next() {
                    if let Ok(seats) = value.parse() {
                        options.seats = seats;
                    }
                }
            }
            "--skip-setup" => options.skip_setup = true,
            "--online-match" => { options.online_match = Some(args.next().unwrap_or_default()); options.skip_setup = true; },
            "--clock" => {
                if let Some(value) = args.next() {
                    if let Err(error) = options.clock.set_control(&value) {
                        eprintln!("{error}");
                    }
                }
            }
            "--initial-ms" => {
                if let Some(value) = args.next() {
                    if let Err(error) = options.clock.set_initial(&value) {
                        eprintln!("{error}");
                    }
                }
            }
            "--increment-ms" | "--delay-ms" => {
                if let Some(value) = args.next() {
                    if let Err(error) = options.clock.set_adjustment(&value) {
                        eprintln!("{error}");
                    }
                }
            }
            "--solo" => options.fill = SeatFill::Solo,
            "--reduced-motion" => options.reduced_motion = true,
            "--theme" => {
                if let Some(theme) = args.next().and_then(|name| parse_local_theme(&name)) {
                    options.theme = theme;
                }
            }
            _ => {}
        }
    }
    options
}

#[rustfmt::skip]
fn parse_game(name: &str) -> SelectedGame {
    match name.to_ascii_lowercase().as_str() {
        "chess" => SelectedGame::Chess, // xtask-allow-game-id: direct Phase 2 local vertical slice wiring.
        #[cfg(feature = "tiles")] // xtask-allow-game-id: optional Phase 3 local vertical slice wiring.
        "tiles" => SelectedGame::Tiles, // xtask-allow-game-id: direct Phase 3 local vertical slice wiring.
        other => {
            eprintln!("unknown game '{other}', defaulting to the first entry");
            SelectedGame::default()
        }
    }
}

fn human_roster(seats: u8) -> SeatRoster {
    SeatRoster::new(
        (0..seats)
            .map(|index| SeatEntry {
                seat: SeatId(index),
                occupant: Occupant::Human(UserId(u128::from(index) + 1)),
                team: None,
            })
            .collect(),
    )
    .expect("local seats are unique")
}

fn play_cues(audio: &mut MacroquadAudioSink, cues: &tabula_presentation::AudioCues) {
    for cue in cues {
        let _ = audio.play(cue);
    }
}

/// A recovery key held across creation cannot immediately submit a new move.
fn fresh_feedback() -> LocalFeedback {
    let mut feedback = LocalFeedback::default();
    suppress_held_activation(&mut feedback);
    feedback
}

/// A timer/bot completion must not turn a held board key into a restart action.
fn suppress_held_activation(feedback: &mut LocalFeedback) {
    for (key, native_key) in [
        (tabula_presentation::Key::Enter, mq::KeyCode::Enter),
        (tabula_presentation::Key::Space, mq::KeyCode::Space),
    ] {
        if mq::is_key_down(native_key) {
            feedback.suppress_opening_input(&tabula_presentation::InputEvent::Key {
                key,
                pressed: true,
            });
        }
    }
}

/// Converts Macroquad's monotonic presentation clock into the renderer frame fact.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::float_arithmetic
)]
fn presentation_now_ms() -> u64 {
    (mq::get_time() * 1_000.0) as u64
}

/// Every fresh local match starts at logical time zero, including recovery.
fn session_elapsed_ms(started_at_ms: u64, now_ms: u64) -> u64 {
    now_ms.saturating_sub(started_at_ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_online_intent_never_falls_back_to_local_authority() {
        let options = parse_options_from(["--online-match".to_owned()].into_iter());
        assert_eq!(options.online_match, Some(String::new()));
        assert!(options.skip_setup);
    }

    #[test]
    fn native_theme_options_keep_the_selected_game_and_invalid_values_keep_light() {
        for (name, expected) in [
            ("light", tabula_design::ThemeKind::Light),
            ("dark", tabula_design::ThemeKind::Dark),
            ("hc-light", tabula_design::ThemeKind::HighContrastLight),
            ("hc-dark", tabula_design::ThemeKind::HighContrastDark),
        ] {
            let options = parse_options_from(
                ["--theme", name, "--seats", "4", "--solo"]
                    .into_iter()
                    .map(str::to_owned),
            );
            assert_eq!(options.theme, expected);
            assert_eq!(options.seats, 4);
            assert_eq!(options.fill, SeatFill::Solo);
        }
        for args in [vec!["--theme"], vec!["--theme", "unknown"]] {
            assert_eq!(
                parse_options_from(args.into_iter().map(str::to_owned)).theme,
                tabula_design::ThemeKind::Light
            );
        }
    }

    #[test]
    fn fresh_local_recovery_resets_elapsed_clock_time_and_clamps_clock_regression() {
        assert_eq!(session_elapsed_ms(900_000, 900_000), 0);
        assert_eq!(session_elapsed_ms(900_000, 900_012), 12);
        assert_eq!(session_elapsed_ms(900_000, 899_999), 0);
    }
    #[test]
    fn local_launch_configures_real_untimed_fischer_and_bronstein_rules() {
        for name in ["untimed", "fischer", "bronstein"] {
            let options = parse_options_from(
                [
                    "--clock",
                    name,
                    "--initial-ms",
                    "120000",
                    "--increment-ms",
                    "5000",
                    "--skip-setup",
                ]
                .into_iter()
                .map(str::to_owned),
            );
            assert!(options.skip_setup);
            let clock = configured_clock(options.clock);
            if name == "untimed" {
                assert_eq!(clock, None);
                continue;
            }
            let clock = clock.unwrap();
            assert_eq!(clock.initial, Millis(120_000));
            assert_eq!(
                clock.control,
                if name == "bronstein" {
                    ClockControl::Bronstein {
                        delay: Millis(5_000),
                    }
                } else {
                    ClockControl::Fischer {
                        increment: Millis(5_000),
                    }
                }
            );
        }
    }

    #[test]
    fn hostile_clock_launch_keeps_the_bounded_default() {
        let options = parse_options_from(
            [
                "--clock",
                "ranked",
                "--initial-ms",
                "0",
                "--delay-ms",
                "18446744073709551615",
            ]
            .into_iter()
            .map(str::to_owned),
        );
        assert_eq!(options.clock, LocalClockOptions::default());
        assert!(!options.skip_setup);
    }
}
