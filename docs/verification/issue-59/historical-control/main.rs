//! Reproducible local renderer control for the permitted Phase-3 projection. // xtask-allow-game-id: Tiles-specific test executable, not platform dispatch.
//!
//! Native flags also work through compile-time `TABULA_BASELINE_OPTIONS` on WASM.
//! This example never exports canonical state or changes production navigation.
//! Timings cover backend method wall time, not GPU completion or whole-process CPU.

#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::float_arithmetic
)]

use core::fmt::Write as _;
use glam::Vec2;
use macroquad::prelude as mq;
use renderer_macroquad::MacroquadRenderer;
use tabula_core::{
    BotLevel, DetRng, InputIndex, MatchSeed, Occupant, SeatEntry, SeatId, SeatRoster, StateHash,
    UserId, Viewer,
};
use tabula_design::{Theme, ThemeKind};
use tabula_game_api::{GameBot, GameModule};
use tabula_game_client::{
    resolve_display_geometry, runtime_ui::parse_local_theme, LocalMatch, LocalMatchError,
};
#[rustfmt::skip]
use tabula_game_tiles::{ // xtask-allow-game-id: direct Phase-3 test-fixture leaf wiring.
    presentation::TilesPresentation as Presentation,
    rules::{Command, TurnPhase}, Config, TilesModule as Module, TilesRules as Rules,
};
use tabula_presentation::{
    Align, Camera2D, FrameCtx, GamePresentation, InputEvent, Key, Layer, RenderCmd,
    RenderListBuilder, Renderer, TextStyleToken,
};

type FixtureMatch = LocalMatch<Rules, Presentation>;
const SEED: [u8; 32] = [47; 32];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Scenario {
    Static,
    Scripted,
}

impl Scenario {
    const fn name(self) -> &'static str {
        match self {
            Self::Static => "static",
            Self::Scripted => "scripted",
        }
    }
}

#[derive(Clone, Debug)]
struct Options {
    theme: ThemeKind,
    reduced_motion: bool,
    scenario: Scenario,
    initial_inputs: u16,
    scripted_inputs: u16,
    interval_ms: u64,
    warmup_ms: u64,
    samples: usize,
    width: i32,
    height: i32,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            theme: ThemeKind::Light,
            reduced_motion: false,
            scenario: Scenario::Static,
            initial_inputs: 24,
            scripted_inputs: 20,
            interval_ms: 750,
            warmup_ms: 3_000,
            samples: 900,
            width: 900,
            height: 720,
        }
    }
}

fn options() -> Result<Options, String> {
    let compiled = option_env!("TABULA_BASELINE_OPTIONS")
        .unwrap_or("")
        .split_whitespace()
        .map(String::from);
    let native = std::env::args().skip(1);
    let mut args = compiled.chain(native);
    let mut options = Options::default();
    while let Some(flag) = args.next() {
        let mut value = || {
            args.next()
                .ok_or_else(|| format!("missing value for {flag}"))
        };
        match flag.as_str() {
            "--theme" => {
                options.theme = parse_local_theme(&value()?).ok_or_else(|| {
                    String::from("theme must be light, dark, hc-light or hc-dark")
                })?;
            }
            "--motion" => {
                options.reduced_motion = match value()?.as_str() {
                    "full" => false,
                    "reduced" => true,
                    _ => return Err(String::from("motion must be full or reduced")),
                };
            }
            "--scenario" => {
                options.scenario = match value()?.as_str() {
                    "static" => Scenario::Static,
                    "scripted" => Scenario::Scripted,
                    _ => return Err(String::from("scenario must be static or scripted")),
                };
            }
            "--initial-inputs" => options.initial_inputs = bounded(&value()?, 0, 100)?,
            "--scripted-inputs" => options.scripted_inputs = bounded(&value()?, 1, 100)?,
            "--interval-ms" => options.interval_ms = bounded(&value()?, 200, 10_000)?,
            "--warmup-ms" => options.warmup_ms = bounded(&value()?, 0, 60_000)?,
            "--samples" => options.samples = bounded(&value()?, 1, 10_000)?,
            "--width" => options.width = bounded(&value()?, 200, 4_096)?,
            "--height" => options.height = bounded(&value()?, 200, 4_096)?,
            _ => return Err(format!("unknown baseline option: {flag}")),
        }
    }
    Ok(options)
}

fn bounded<T>(value: &str, minimum: T, maximum: T) -> Result<T, String>
where
    T: core::str::FromStr + PartialOrd + core::fmt::Display + Copy,
{
    let parsed = value
        .parse::<T>()
        .map_err(|_| format!("invalid number: {value}"))?;
    if parsed < minimum || parsed > maximum {
        return Err(format!("number must be within {minimum}..={maximum}"));
    }
    Ok(parsed)
}

fn window_conf() -> mq::Conf {
    let options = options().unwrap_or_default();
    mq::Conf {
        window_title: String::from("Tabula renderer baseline"),
        window_width: options.width,
        window_height: options.height,
        high_dpi: true,
        ..mq::Conf::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut renderer = MacroquadRenderer::new();
    let result = match options() {
        Ok(options) => run(&mut renderer, &options).await,
        Err(error) => Err(error),
    };
    if let Err(error) = result {
        macroquad::logging::info!(
            "TABULA_BASELINE {{\"kind\":\"failure\",\"error\":{}}}",
            json_string(&error)
        );
        show_failure(&mut renderer, &error).await;
    }
}

// Keep the sampling boundaries visible in one imperative loop.
#[allow(clippy::too_many_lines)]
async fn run(renderer: &mut MacroquadRenderer, options: &Options) -> Result<(), String> {
    let mut theme = Theme::by_kind(options.theme);
    if options.reduced_motion {
        theme.motion.reduced.duration_scale = tabula_design::Percent::new(0).expect("zero scale");
    }
    let (viewport, dpi) = display()?;
    let initial_frame = FrameCtx::new(viewport, dpi, 0, theme);
    let mut match_ = FixtureMatch::new(
        &Config {
            turn_deadline_ms: 0,
        },
        &roster(),
        MatchSeed::from_bytes(SEED),
        Viewer::Seat(SeatId(0)),
    )
    .map_err(|error| format!("fixture create: {error:?}"))?;
    match_.local_mut().set_viewport(viewport);
    let bot = Module::bot(BotLevel::Easy).ok_or("fixture bot is unavailable")?;
    let mut bot_rng = DetRng::for_input(&MatchSeed::from_bytes(SEED), InputIndex(u64::MAX));
    for _ in 0..options.initial_inputs {
        bot_step(&mut match_, bot.as_ref(), &mut bot_rng, &initial_frame)?;
    }
    let fixture_hash = checkpoint(&match_);
    macroquad::logging::info!(
        "TABULA_BASELINE {{\"kind\":\"ready\",\"commit\":{},\"build\":{},\"target\":{},\"backend\":\"macroquad-0.4.16/miniquad-0.4.11\",\"historical_visual_control\":true,\"asset_support\":\"absent\",\"pack\":{},\"asset_hashes\":[],\"seed_byte\":47,\"seats\":3,\"initial_inputs\":{},\"fixture_hash\":{},\"scenario\":{},\"theme\":{},\"motion\":{},\"viewport\":[{},{}],\"dpi\":{},\"warmup_ms\":{},\"samples\":{},\"interval_ms\":{},\"scripted_inputs\":{}}}",
        json_string(option_env!("TABULA_BASELINE_COMMIT").unwrap_or("UNRECORDED")),
        json_string(option_env!("TABULA_BASELINE_BUILD").unwrap_or("UNRECORDED")),
        json_string(std::env::consts::ARCH), json_string(&Presentation::asset_pack().to_string()),
        options.initial_inputs, json_string(&fixture_hash), json_string(options.scenario.name()),
        json_string(&format!("{:?}", options.theme)),
        json_string(if options.reduced_motion { "reduced" } else { "full" }),
        viewport.size().x, viewport.size().y, dpi.get(), options.warmup_ms, options.samples,
        options.interval_ms, options.scripted_inputs,
    );
    let running = mq::get_time();
    let mut measurements = Measurements::new(options.samples);
    let mut previous_frame = running;
    let mut next_step = options.warmup_ms + options.interval_ms;
    let mut script_steps = 0_u16;
    let mut previewed = false;
    let mut uncontrolled_input_events = 0_u64;
    let mut reported = false;
    loop {
        let frame_start = mq::get_time();
        let now_ms = ((frame_start - running) * 1_000.0) as u64;
        let interval_ms = (frame_start - previous_frame) * 1_000.0;
        previous_frame = frame_start;
        let Some((viewport, dpi)) = resolve_display_geometry(
            mq::screen_width(),
            mq::screen_height(),
            mq::screen_dpi_scale(),
        ) else {
            mq::next_frame().await;
            continue;
        };
        let frame = renderer.begin_frame(viewport, dpi, now_ms, theme);
        match_.local_mut().set_viewport(viewport);
        match_
            .advance_frame(&frame)
            .map_err(|error| format!("advance: {error:?}"))?;
        match_.set_viewer(Viewer::Seat(match_.view().turn));
        for event in renderer.drain_input() {
            if options.scenario == Scenario::Static {
                continue;
            }
            // Pointer motion alone is still counted: it can change local preview/camera pixels.
            uncontrolled_input_events += 1;
            match match_.handle_presentation_input(&event, &frame) {
                Ok(_) => {}
                Err(LocalMatchError::Rejected(error)) => macroquad::logging::info!(
                    "TABULA_BASELINE {{\"kind\":\"interaction-rejected\",\"code\":{}}}",
                    json_string(&format!("{:?}", error.code)),
                ),
                Err(error) => return Err(format!("interaction: {error:?}")),
            }
        }
        if options.scenario == Scenario::Scripted && script_steps < options.scripted_inputs {
            // A real presenter key rotates the local preview before the accepted placement.
            if !previewed
                && now_ms >= next_step.saturating_sub(300)
                && match_.view().phase == TurnPhase::PlaceTile
            {
                for pressed in [true, false] {
                    match_
                        .handle_presentation_input(
                            &InputEvent::Key {
                                key: Key::Space,
                                pressed,
                            },
                            &frame,
                        )
                        .map_err(|error| format!("preview rotation: {error:?}"))?;
                }
                previewed = true;
            }
            if now_ms >= next_step {
                bot_step(&mut match_, bot.as_ref(), &mut bot_rng, &frame)?;
                script_steps += 1;
                next_step += options.interval_ms;
                previewed = false;
            }
        }
        match_.drain_notices().for_each(drop);
        match_.drain_bot_requests().for_each(drop);
        let list = match_.present(&frame);
        let cpu_started = mq::get_time();
        renderer
            .submit(&list)
            .map_err(|error| format!("render: {error:?}"))?;
        renderer
            .end_frame()
            .map_err(|error| format!("end frame: {error:?}"))?;
        let cpu_us = (mq::get_time() - cpu_started) * 1_000_000.0;
        if now_ms >= options.warmup_ms && !reported {
            if measurements.cpu_us.len() == 10_000 {
                return Err(String::from(
                    "script did not complete within 10000 rendered sample frames",
                ));
            }
            measurements.record(cpu_us, interval_ms);
            let script_complete =
                options.scenario != Scenario::Scripted || script_steps == options.scripted_inputs;
            if measurements.complete() && script_complete {
                let (place, claim, skip) = accepted_kinds(&match_);
                macroquad::logging::info!(
                    "TABULA_BASELINE {{\"kind\":\"measurement\",\"samples\":{},\"submit_end_cpu_us\":{},\"frame_interval_ms\":{},\"elapsed_ms\":{},\"accepted_inputs\":{},\"placements\":{},\"claims\":{},\"skips\":{},\"scripted_steps\":{},\"script_complete\":{},\"uncontrolled_input_events\":{},\"final_checkpoint\":{},\"board_cells\":{},\"viewport\":[{},{}],\"dpi\":{}}}",
                    measurements.cpu_us.len(), summary(&measurements.cpu_us), summary(&measurements.interval_ms),
                    now_ms, match_.replay_trace().accepted_inputs().len(), place, claim, skip,
                    script_steps, script_complete,
                    uncontrolled_input_events, json_string(&checkpoint(&match_)),
                    match_.view().board.len(), viewport.size().x, viewport.size().y, dpi.get(),
                );
                reported = true;
            }
        }
        mq::next_frame().await;
    }
}

fn display() -> Result<(tabula_presentation::Viewport, tabula_presentation::Dpi), String> {
    resolve_display_geometry(
        mq::screen_width(),
        mq::screen_height(),
        mq::screen_dpi_scale(),
    )
    .ok_or_else(|| String::from("display has no finite positive extent/DPI"))
}

fn roster() -> SeatRoster {
    SeatRoster::new(
        (0..3)
            .map(|seat| SeatEntry {
                seat: SeatId(seat),
                occupant: Occupant::Human(UserId(u128::from(seat) + 1)),
                team: None,
            })
            .collect(),
    )
    .expect("three unique fixture seats")
}

fn bot_step(
    match_: &mut FixtureMatch,
    bot: &dyn GameBot<Rules>,
    rng: &mut DetRng,
    frame: &FrameCtx,
) -> Result<(), String> {
    let seat = match_.view().turn;
    match_.set_viewer(Viewer::Seat(seat));
    let command = bot
        .choose(match_.view(), seat, rng)
        .ok_or("fixture bot produced no command")?;
    match_
        .submit_bot_move(seat, command, frame)
        .map_err(|error| format!("fixture command rejected: {error:?}"))?;
    Ok(())
}

fn checkpoint(match_: &FixtureMatch) -> String {
    let hash = match_
        .replay_trace()
        .accepted_inputs()
        .last()
        .map_or(match_.replay_trace().initial_state_hash(), |entry| {
            entry.state_hash()
        });
    hash_hex(hash)
}

fn hash_hex(hash: StateHash) -> String {
    let mut hex = String::with_capacity(64);
    for byte in hash.0 {
        write!(hex, "{byte:02x}").expect("writing to a String is infallible");
    }
    hex
}

fn accepted_kinds(match_: &FixtureMatch) -> (usize, usize, usize) {
    let mut counts = (0, 0, 0);
    for entry in match_.replay_trace().accepted_inputs() {
        if let tabula_game_api::Input::Player { command, .. } = entry.input() {
            match command {
                Command::PlaceTile { .. } => counts.0 += 1,
                Command::PlaceMeeple { .. } => counts.1 += 1,
                Command::SkipMeeple => counts.2 += 1,
            }
        }
    }
    counts
}

struct Measurements {
    cpu_us: Vec<f64>,
    interval_ms: Vec<f64>,
    limit: usize,
}

impl Measurements {
    fn new(limit: usize) -> Self {
        Self {
            cpu_us: Vec::with_capacity(limit),
            interval_ms: Vec::with_capacity(limit),
            limit,
        }
    }
    fn record(&mut self, cpu_us: f64, interval_ms: f64) {
        self.cpu_us.push(cpu_us);
        self.interval_ms.push(interval_ms);
    }
    fn complete(&self) -> bool {
        self.cpu_us.len() >= self.limit
    }
}

fn summary(samples: &[f64]) -> String {
    let mut sorted = samples.to_vec();
    sorted.sort_by(f64::total_cmp);
    // Nearest-rank percentiles are explicit and stable even for a one-frame sample.
    let rank = |percent: usize| sorted[(sorted.len() * percent).div_ceil(100).saturating_sub(1)];
    format!(
        "{{\"mean\":{:.3},\"p50\":{:.3},\"p95\":{:.3},\"max\":{:.3}}}",
        samples.iter().sum::<f64>() / samples.len() as f64,
        rank(50),
        rank(95),
        sorted[sorted.len() - 1]
    )
}

fn json_string(value: &str) -> String {
    let mut escaped = String::from("\"");
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            character if character.is_control() => {
                write!(escaped, "\\u{:04x}", u32::from(character))
                    .expect("writing to a String is infallible");
            }
            character => escaped.push(character),
        }
    }
    escaped.push('"');
    escaped
}

async fn show_failure(renderer: &mut MacroquadRenderer, error: &str) {
    loop {
        if let Ok((viewport, dpi)) = display() {
            let frame = renderer.begin_frame(viewport, dpi, 0, Theme::by_kind(ThemeKind::Light));
            let mut builder = RenderListBuilder::new(Camera2D::default());
            for (text, y) in [("Renderer baseline stopped", 30.0), (error, 75.0)] {
                let _ = builder.push(RenderCmd::Text {
                    text: text.to_owned(),
                    at: Vec2::new(24.0, y),
                    style: TextStyleToken::BodyMd,
                    align: Align::Start,
                    max_width: tabula_design::Positive::new((viewport.size().x - 48.0).max(1.0))
                        .ok(),
                    color: frame.theme().color.danger,
                    layer: Layer::HUD,
                    z: 0,
                });
            }
            if let Ok(list) = builder.finish() {
                let _ = renderer.submit(&list);
            }
            let _ = renderer.end_frame();
        }
        mq::next_frame().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measurement_percentiles_use_nearest_rank_on_nonempty_samples() {
        assert_eq!(
            summary(&[9.0, 1.0, 3.0, 7.0, 5.0]),
            "{\"mean\":5.000,\"p50\":5.000,\"p95\":9.000,\"max\":9.000}"
        );
        assert_eq!(
            summary(&[4.0]),
            "{\"mean\":4.000,\"p50\":4.000,\"p95\":4.000,\"max\":4.000}"
        );
        assert!(bounded::<usize>("10001", 1, 10_000).is_err());
        assert_eq!(json_string("a\n\"\\"), "\"a\\u000a\\\"\\\\\"");
    }

    #[test]
    fn reachable_historical_fixture_checkpoint() {
        let (viewport, dpi) = resolve_display_geometry(900.0, 720.0, 1.0).unwrap();
        let frame = FrameCtx::new(viewport, dpi, 0, Theme::by_kind(ThemeKind::Light));
        let bot = Module::bot(BotLevel::Easy).unwrap();
        let mut match_ = FixtureMatch::new(
            &Config {
                turn_deadline_ms: 0,
            },
            &roster(),
            MatchSeed::from_bytes(SEED),
            Viewer::Seat(SeatId(0)),
        )
        .unwrap();
        match_.local_mut().set_viewport(viewport);
        let mut rng = DetRng::for_input(&MatchSeed::from_bytes(SEED), InputIndex(u64::MAX));
        for _ in 0..24 {
            bot_step(&mut match_, bot.as_ref(), &mut rng, &frame).unwrap();
        }
        assert_eq!(match_.replay_trace().accepted_inputs().len(), 24);
        assert!(match_.view().board.len() > 1);
        let (placements, claims, skips) = accepted_kinds(&match_);
        assert!(claims > 0, "fixture must reach a real follower claim");
        println!("HISTORICAL_CONTROL_FIXTURE checkpoint={} inputs=24 placements={placements} claims={claims} skips={skips} board_cells={}", checkpoint(&match_), match_.view().board.len());
    }
}
