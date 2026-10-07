//! Issue-60 isolated native authority and renderer fixture (doc 00 I-5/I-10).
//!
//! `--export DIR` writes the exact issue-59 projected control and Rust-owned schemas.
//! `--stdio` accepts bounded JSON lines for one native local-match owner; no socket,
//! production navigation, canonical State DTO, or JavaScript game logic is introduced.

#![forbid(unsafe_code)]
#![allow(clippy::float_arithmetic)]

use core::fmt::Write as _;
use std::{
    collections::BTreeMap,
    io::{BufRead, Read},
    path::Path,
};

use glam::Vec2;
use serde::Deserialize;
use serde_json::{json, Map, Value};
use tabula_assets::{AssetFile, AssetPackManifest};
use tabula_core::{
    BotLevel, DetRng, InputIndex, MatchSeed, Occupant, SeatEntry, SeatId, SeatRoster, UserId,
    Viewer,
};
use tabula_design::{Color, TextStyleToken, Theme, ThemeKind};
use tabula_game_api::{GameBot, GameModule};
use tabula_game_client::{LocalMatch, LocalMatchError};
use tabula_game_tiles::{
    presentation::{fixture, TilesPresentation},
    rules::TurnPhase,
    Config, TilesModule, TilesRules,
};
use tabula_presentation::{
    Align, Dpi, FrameCtx, GamePresentation, InputEvent, Key, Paint, PointerButton, PointerPhase,
    PointerPosition, Rect, RenderCmd, Viewport,
};

type FixtureMatch = LocalMatch<TilesRules, TilesPresentation>;
const SEED: [u8; 32] = [47; 32];
const INITIAL: usize = 24;
const SCRIPTED: u16 = 20;
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
const INITIAL_CHECKPOINT: &str = "e4ed3465b826a55c12d68d8f8bcbef5422fa5d128a251da1f4f659470060d032";
const FINAL_CHECKPOINT: &str = "b7e81e41d5bd48f076a736858b6855b663591034127628be52b887ae1ce19a73";

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Scheme {
    Light,
    Dark,
    HcLight,
    HcDark,
}

impl Scheme {
    const fn theme(self) -> Theme {
        Theme::by_kind(match self {
            Self::Light => ThemeKind::Light,
            Self::Dark => ThemeKind::Dark,
            Self::HcLight => ThemeKind::HighContrastLight,
            Self::HcDark => ThemeKind::HighContrastDark,
        })
    }

    const fn name(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
            Self::HcLight => "hc-light",
            Self::HcDark => "hc-dark",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Scenario {
    Static,
    Interactive,
    Scripted,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
enum Request {
    Init {
        session_id: String,
        generation: u64,
        revision: u64,
        viewport: [f32; 2],
        dpi: f32,
        theme: Scheme,
        reduced_motion: bool,
        scenario: Scenario,
    },
    Frame {
        session_id: String,
        generation: u64,
        revision: u64,
        now_ms: u64,
        viewport: [f32; 2],
        dpi: f32,
        theme: Scheme,
        reduced_motion: bool,
    },
    Input {
        session_id: String,
        generation: u64,
        revision: u64,
        now_ms: u64,
        input: RawInput,
    },
    Dispose {
        session_id: String,
        generation: u64,
        revision: u64,
    },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum RawInput {
    Pointer {
        position: [f32; 2],
        button: RawButton,
        phase: RawPhase,
    },
    Key {
        key: RawKey,
        pressed: bool,
    },
    Focus {
        focused: bool,
    },
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RawButton {
    Primary,
    Secondary,
    Middle,
}
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RawPhase {
    Down,
    Move,
    Up,
    Cancel,
}
#[derive(Clone, Copy, Debug, Deserialize)]
enum RawKey {
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Enter,
    Space,
    Escape,
    Tab,
}

impl RawInput {
    fn resolve(self) -> Result<InputEvent, String> {
        Ok(match self {
            Self::Pointer {
                position,
                button,
                phase,
            } => {
                if position
                    .iter()
                    .any(|axis| !axis.is_finite() || axis.abs() > 1_000_000.0)
                {
                    return Err(String::from("invalid pointer coordinate"));
                }
                InputEvent::Pointer {
                    position: PointerPosition::new(Vec2::from_array(position))
                        .map_err(|error| error.to_string())?,
                    button: match button {
                        RawButton::Primary => PointerButton::Primary,
                        RawButton::Secondary => PointerButton::Secondary,
                        RawButton::Middle => PointerButton::Middle,
                    },
                    phase: match phase {
                        RawPhase::Down => PointerPhase::Down,
                        RawPhase::Move => PointerPhase::Move,
                        RawPhase::Up => PointerPhase::Up,
                        RawPhase::Cancel => PointerPhase::Cancel,
                    },
                }
            }
            Self::Key { key, pressed } => InputEvent::Key {
                key: match key {
                    RawKey::ArrowUp => Key::ArrowUp,
                    RawKey::ArrowDown => Key::ArrowDown,
                    RawKey::ArrowLeft => Key::ArrowLeft,
                    RawKey::ArrowRight => Key::ArrowRight,
                    RawKey::Enter => Key::Enter,
                    RawKey::Space => Key::Space,
                    RawKey::Escape => Key::Escape,
                    RawKey::Tab => Key::Tab,
                },
                pressed,
            },
            Self::Focus { focused } => InputEvent::Focus(focused),
        })
    }
}

struct Session {
    match_: FixtureMatch,
    frame: FrameCtx,
    scheme: Scheme,
    generation: u64,
    revision: u64,
    scenario: Scenario,
    reduced_motion: bool,
    bot: Box<dyn GameBot<TilesRules>>,
    bot_rng: DetRng,
    script_steps: u16,
    previewed: bool,
}

impl Session {
    // The copy is stored as the session's owned frame snapshot.
    #[allow(clippy::large_types_passed_by_value)]
    fn new(
        generation: u64,
        frame: FrameCtx,
        scheme: Scheme,
        reduced: bool,
        scenario: Scenario,
    ) -> Result<Self, String> {
        let roster = SeatRoster::new(
            (0..3)
                .map(|seat| SeatEntry {
                    seat: SeatId(seat),
                    occupant: Occupant::Human(UserId(u128::from(seat) + 1)),
                    team: None,
                })
                .collect(),
        )
        .map_err(|error| format!("roster: {error:?}"))?;
        let mut match_ = FixtureMatch::new(
            &Config {
                turn_deadline_ms: 0,
            },
            &roster,
            MatchSeed::from_bytes(SEED),
            Viewer::Seat(SeatId(0)),
        )
        .map_err(|error| format!("create: {error:?}"))?;
        match_.local_mut().set_frame_context(&frame);
        match_.local_mut().set_reduced_motion(reduced);
        let bot = TilesModule::bot(BotLevel::Easy).ok_or("fixture bot unavailable")?;
        let mut bot_rng = DetRng::for_input(&MatchSeed::from_bytes(SEED), InputIndex(u64::MAX));
        for _ in 0..INITIAL {
            bot_step(&mut match_, bot.as_ref(), &mut bot_rng, &frame)?;
        }
        if checkpoint(&match_) != INITIAL_CHECKPOINT {
            return Err(String::from("issue-59 initial checkpoint mismatch"));
        }
        Ok(Self {
            match_,
            frame,
            scheme,
            generation,
            revision: 0,
            scenario,
            reduced_motion: reduced,
            bot,
            bot_rng,
            script_steps: 0,
            previewed: false,
        })
    }

    #[allow(clippy::large_types_passed_by_value)]
    fn update_frame(
        &mut self,
        frame: FrameCtx,
        scheme: Scheme,
        reduced: bool,
    ) -> Result<Vec<String>, String> {
        if frame.now_ms() < self.frame.now_ms() {
            return Err(String::from("presentation time regressed"));
        }
        self.frame = frame;
        self.scheme = scheme;
        self.reduced_motion = reduced;
        self.match_.local_mut().set_frame_context(&frame);
        self.match_.local_mut().set_reduced_motion(reduced);
        let mut cues: Vec<String> = self
            .match_
            .advance_frame(&frame)
            .map_err(|error| format!("advance: {error:?}"))?
            .into_iter()
            .map(|cue| cue.id().to_owned())
            .collect();
        self.match_
            .set_viewer(Viewer::Seat(self.match_.view().turn));
        if self.scenario == Scenario::Scripted && self.script_steps < SCRIPTED {
            let next_step = 3_750 + u64::from(self.script_steps) * 750;
            if !self.previewed
                && frame.now_ms() >= next_step.saturating_sub(300)
                && self.match_.view().phase == TurnPhase::PlaceTile
            {
                for pressed in [true, false] {
                    self.match_
                        .handle_presentation_input(
                            &InputEvent::Key {
                                key: Key::Space,
                                pressed,
                            },
                            &frame,
                        )
                        .map_err(|error| format!("preview: {error:?}"))?;
                }
                self.previewed = true;
            }
            // At most one step per host frame, exactly as issue-59's baseline loop.
            if frame.now_ms() >= next_step {
                cues.extend(bot_step(
                    &mut self.match_,
                    self.bot.as_ref(),
                    &mut self.bot_rng,
                    &frame,
                )?);
                self.script_steps += 1;
                self.previewed = false;
            }
        }
        Ok(cues)
    }

    fn frame_json(&mut self) -> Value {
        let list = self.match_.present(&self.frame);
        let camera = list.camera();
        let local = self.match_.local_mut().clone();
        let a11y = TilesPresentation::a11y(self.match_.view(), &local);
        json!({"at_ms":self.frame.now_ms(),"checkpoint":checkpoint(&self.match_),
            "input_count":self.match_.replay_trace().accepted_inputs().len(),
            "board_cells":self.match_.view().board.len(),"script_steps":self.script_steps,
            "viewport":self.frame.viewport().size().to_array(),"dpi":self.frame.dpi().get(),
            "theme":self.scheme.name(),"reduced_motion":self.reduced_motion,
            "camera":{"origin":camera.origin().to_array(),"zoom":camera.zoom()},
            "commands":list.commands().iter().map(lower_command).collect::<Vec<_>>(),"a11y":a11y.status})
    }
}

#[derive(Default)]
struct Authority {
    sessions: BTreeMap<String, Session>,
    generations: BTreeMap<String, u64>,
}

impl Authority {
    // Keep lifecycle ordering and stale checks visible at this one imperative boundary.
    #[allow(clippy::too_many_lines)]
    fn execute(&mut self, request: Request) -> Result<Value, String> {
        match request {
            Request::Init {
                session_id,
                generation,
                revision,
                viewport,
                dpi,
                theme,
                reduced_motion,
                scenario,
            } => {
                validate_identity(&session_id, generation, revision)?;
                if revision != 0
                    || self.sessions.contains_key(&session_id)
                    || self
                        .generations
                        .get(&session_id)
                        .is_some_and(|previous| generation <= *previous)
                {
                    return Err(String::from("stale or duplicate init"));
                }
                if self.sessions.len() >= 8
                    || (!self.generations.contains_key(&session_id)
                        && self.generations.len() >= 128)
                {
                    return Err(String::from("bounded fixture session capacity exceeded"));
                }
                let mut session = Session::new(
                    generation,
                    frame(viewport, dpi, 0, theme)?,
                    theme,
                    reduced_motion,
                    scenario,
                )?;
                let result = response(&session_id, &mut session, "initialized", None, Vec::new());
                self.generations.insert(session_id.clone(), generation);
                self.sessions.insert(session_id, session);
                Ok(result)
            }
            Request::Frame {
                session_id,
                generation,
                revision,
                now_ms,
                viewport,
                dpi,
                theme,
                reduced_motion,
            } => {
                let next_frame = frame(viewport, dpi, now_ms, theme)?;
                let session = self.session(&session_id, generation, revision)?;
                let cues = session.update_frame(next_frame, theme, reduced_motion)?;
                session.revision += 1;
                Ok(response(&session_id, session, "local", None, cues))
            }
            Request::Input {
                session_id,
                generation,
                revision,
                now_ms,
                input,
            } => {
                let input = input.resolve()?;
                let session = self.session(&session_id, generation, revision)?;
                if session.scenario == Scenario::Static {
                    return Err(String::from("static control accepts no host input"));
                }
                let next_frame = frame(
                    session.frame.viewport().size().to_array(),
                    session.frame.dpi().get(),
                    now_ms,
                    session.scheme,
                )?;
                if now_ms < session.frame.now_ms() {
                    return Err(String::from("presentation time regressed"));
                }
                session.frame = next_frame;
                session.match_.local_mut().set_frame_context(&next_frame);
                session
                    .match_
                    .set_viewer(Viewer::Seat(session.match_.view().turn));
                let accepted_before = session.match_.replay_trace().accepted_inputs().len();
                let (outcome, error, cues) = match session
                    .match_
                    .handle_presentation_input(&input, &next_frame)
                {
                    Ok(cues) => (
                        if session.match_.replay_trace().accepted_inputs().len() > accepted_before {
                            "accepted"
                        } else {
                            "local"
                        },
                        None,
                        cues.into_iter().map(|cue| cue.id().to_owned()).collect(),
                    ),
                    Err(LocalMatchError::Rejected(error)) => {
                        ("rejected", Some(format!("{:?}", error.code)), Vec::new())
                    }
                    Err(error) => return Err(format!("input: {error:?}")),
                };
                session.revision += 1;
                Ok(response(&session_id, session, outcome, error, cues))
            }
            Request::Dispose {
                session_id,
                generation,
                revision,
            } => {
                self.session(&session_id, generation, revision)?;
                self.sessions.remove(&session_id);
                Ok(
                    json!({"ok":true,"outcome":"disposed","session_id":session_id,"generation":generation,
                    "revision":revision,"error":null}),
                )
            }
        }
    }

    fn session(
        &mut self,
        id: &str,
        generation: u64,
        revision: u64,
    ) -> Result<&mut Session, String> {
        validate_identity(id, generation, revision)?;
        let session = self.sessions.get_mut(id).ok_or("session not mounted")?;
        if session.generation != generation || session.revision != revision {
            return Err(String::from("stale session generation or revision"));
        }
        if revision == MAX_SAFE_INTEGER {
            return Err(String::from("revision exhausted"));
        }
        Ok(session)
    }
}

fn validate_identity(id: &str, generation: u64, revision: u64) -> Result<(), String> {
    if id.is_empty()
        || id.len() > 64
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
        || generation == 0
        || generation > MAX_SAFE_INTEGER
        || revision > MAX_SAFE_INTEGER
    {
        return Err(String::from("invalid session identity"));
    }
    Ok(())
}

fn frame(viewport: [f32; 2], dpi: f32, now_ms: u64, scheme: Scheme) -> Result<FrameCtx, String> {
    if viewport
        .iter()
        .any(|axis| !axis.is_finite() || !(200.0..=4096.0).contains(axis))
        || !dpi.is_finite()
        || !(0.5..=4.0).contains(&dpi)
        || now_ms > u64::from(u32::MAX)
    {
        return Err(String::from("invalid frame geometry or time"));
    }
    Ok(FrameCtx::new(
        Viewport::new(Vec2::from_array(viewport))
            .map_err(|error| format!("viewport: {error:?}"))?,
        Dpi::new(dpi).map_err(|error| format!("dpi: {error:?}"))?,
        now_ms,
        scheme.theme(),
    ))
}

// Values belong to the response even though serde's construction temporarily borrows them.
#[allow(clippy::needless_pass_by_value)]
fn response(
    id: &str,
    session: &mut Session,
    outcome: &str,
    error: Option<String>,
    cues: Vec<String>,
) -> Value {
    json!({"ok":true,"outcome":outcome,"error":error,"cues":cues,
        "envelope":{"schema_version":1,"session_id":id,"generation":session.generation,
        "revision":session.revision,"frame":session.frame_json()}})
}

fn bot_step(
    match_: &mut FixtureMatch,
    bot: &dyn GameBot<TilesRules>,
    rng: &mut DetRng,
    frame: &FrameCtx,
) -> Result<Vec<String>, String> {
    let seat = match_.view().turn;
    match_.set_viewer(Viewer::Seat(seat));
    let command = bot
        .choose(match_.view(), seat, rng)
        .ok_or("fixture bot produced no command")?;
    match_
        .submit_bot_move(seat, command, frame)
        .map(|cues| cues.into_iter().map(|cue| cue.id().to_owned()).collect())
        .map_err(|error| format!("fixture command rejected: {error:?}"))
}

fn checkpoint(match_: &FixtureMatch) -> String {
    let hash = match_
        .replay_trace()
        .accepted_inputs()
        .last()
        .map_or(match_.replay_trace().initial_state_hash(), |entry| {
            entry.state_hash()
        });
    let mut result = String::with_capacity(64);
    for byte in hash.0 {
        write!(result, "{byte:02x}").expect("String write is infallible");
    }
    result
}

fn color(value: Color) -> Value {
    json!([value.red(), value.green(), value.blue(), value.alpha()])
}
fn rect(value: Rect) -> Value {
    json!([
        value.origin().x,
        value.origin().y,
        value.size().x,
        value.size().y
    ])
}
fn border(value: tabula_presentation::Border) -> Value {
    json!({"width":value.width(),"color":color(value.color())})
}
fn paint(value: &Paint) -> Value {
    match value {
        Paint::Solid(value) => json!({"kind":"solid","color":color(*value)}),
        Paint::LinearGradient(value) => {
            json!({"kind":"linear_gradient","from":value.from().to_array(),"to":value.to().to_array(),
            "stops":value.stops().iter().map(|stop| json!({"offset":stop.offset(),"color":color(stop.color())})).collect::<Vec<_>>() })
        }
    }
}

// Field names and exported descriptors share this Rust declaration. JavaScript consumes the
// generated descriptor rather than maintaining a parallel Rust/TS render variant union.
macro_rules! command_contract {
    ($($kind:ident => [$($field:ident),*]),* $(,)?) => {
        fn command_fields(kind: &str) -> Option<&'static [&'static str]> {
            match kind { $(stringify!($kind) => Some(&["kind", "layer", "z", $(stringify!($field)),*]),)* _ => None }
        }
        fn command_schema() -> Value { json!({ $(stringify!($kind): command_fields(stringify!($kind)),)* }) }
    };
}
command_contract! {
    sprite => [asset, rect, tint, rotation, pivot],
    rect => [rect, radii, fill, border],
    text => [text, at, style, align, max_width, color],
    path => [points, stroke, closed, fill],
    push_clip => [rect], pop_clip => [], push_transform => [matrix], pop_transform => [],
    push_opacity => [opacity], pop_opacity => [],
}

fn lower_command(command: &RenderCmd) -> Value {
    let (kind, layer, z, fields) = match command {
        RenderCmd::Sprite {
            asset,
            rect: area,
            tint,
            rotation,
            pivot,
            layer,
            z,
        } => (
            "sprite",
            layer,
            z,
            json!({"asset":asset.as_str(),"rect":rect(*area),"tint":color(*tint),"rotation":rotation,"pivot":pivot.to_array()}),
        ),
        RenderCmd::Rect {
            rect: area,
            radii,
            fill,
            border: edge,
            layer,
            z,
        } => (
            "rect",
            layer,
            z,
            json!({"rect":rect(*area),"radii":[radii.top_left(),radii.top_right(),radii.bottom_right(),radii.bottom_left()],
                "fill":fill.as_ref().map(paint),"border":edge.map(border)}),
        ),
        RenderCmd::Text {
            text,
            at,
            style,
            align,
            max_width,
            color: tint,
            layer,
            z,
        } => (
            "text",
            layer,
            z,
            json!({"text":text,"at":at.to_array(),"style":format!("{style:?}"),
                "align":match align {Align::Start=>"start",Align::Center=>"center",Align::End=>"end"},
                "max_width":max_width.map(tabula_design::Positive::get),"color":color(*tint)}),
        ),
        RenderCmd::Path {
            points,
            stroke,
            closed,
            fill,
            layer,
            z,
        } => (
            "path",
            layer,
            z,
            json!({"points":points.iter().map(Vec2::to_array).collect::<Vec<_>>(),"stroke":border(*stroke),
                "closed":closed,"fill":fill.as_ref().map(paint)}),
        ),
        RenderCmd::PushClip {
            rect: area,
            layer,
            z,
        } => ("push_clip", layer, z, json!({"rect":rect(*area)})),
        RenderCmd::PopClip { layer, z } => ("pop_clip", layer, z, json!({})),
        RenderCmd::PushTransform { matrix, layer, z } => (
            "push_transform",
            layer,
            z,
            json!({"matrix":matrix.to_cols_array()}),
        ),
        RenderCmd::PopTransform { layer, z } => ("pop_transform", layer, z, json!({})),
        RenderCmd::PushOpacity { opacity, layer, z } => {
            ("push_opacity", layer, z, json!({"opacity":opacity.get()}))
        }
        RenderCmd::PopOpacity { layer, z } => ("pop_opacity", layer, z, json!({})),
    };
    let mut result = fields.as_object().expect("literal object").clone();
    result.insert(String::from("kind"), json!(kind));
    result.insert(String::from("layer"), json!(layer.0));
    result.insert(String::from("z"), json!(z));
    debug_assert_eq!(
        result.len(),
        command_fields(kind)
            .expect("closed lowering vocabulary")
            .len()
    );
    Value::Object(result)
}

fn contract() -> Value {
    json!({"schema_version":1,"commands":command_schema(),
        "color_encoding":"rgba-u8","coordinate_units":"logical-css-pixels",
        "transform_layout":"column-major-affine2","opacity_encoding":"unit-interval",
        "input":{"pointer":["kind","position","button","phase"],"key":["kind","key","pressed"],"focus":["kind","focused"]},
        "keys":["ArrowUp","ArrowDown","ArrowLeft","ArrowRight","Enter","Space","Escape","Tab"],
        "pointer_buttons":["primary","secondary","middle"],"pointer_phases":["down","move","up","cancel"],
        "themes":["light","dark","hc-light","hc-dark"],"frame_fields":["at_ms","checkpoint","input_count","board_cells","script_steps","viewport","dpi","theme","reduced_motion","camera","commands","a11y"],
        "source":"xtask/examples/embedding_fixture.rs","production_wire_protocol":false})
}

fn assets() -> Result<Value, String> {
    let manifest =
        AssetPackManifest::from_toml(fixture::MANIFEST).map_err(|error| error.to_string())?;
    let files = manifest.files().iter().map(|file| {
        let bytes = if file.density().map(tabula_assets::AssetDensity::get) == Some(1) { fixture::ATLAS_1X } else { fixture::ATLAS_2X };
        verify_atlas_bytes(file, bytes)?;
        Ok(json!({"name":file.name().as_str(),"path":file.path().as_str(),"hash":file.hash().to_string(),
            "bytes":file.bytes().get(),"density":file.density().map(tabula_assets::AssetDensity::get)}))
    }).collect::<Result<Vec<_>, String>>()?;
    let resources = manifest.resources().iter().map(|resource| json!({"id":resource.id().as_str(),
        "variants":(0..resource.variant_count()).map(|index| { let variant=resource.variant(index).expect("valid index");
            json!({"file":variant.file().as_str(),"region":variant.region().map(|r|[r.x(),r.y(),r.width(),r.height()])}) }).collect::<Vec<_>>() })).collect::<Vec<_>>();
    Ok(json!({"pack":manifest.pack_ref().to_string(),"files":files,"resources":resources}))
}

fn verify_atlas_bytes(file: &AssetFile, bytes: &[u8]) -> Result<(), String> {
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) != file.bytes().get() {
        return Err(format!("fixture asset byte size mismatch: {}", file.name()));
    }
    if blake3::hash(bytes).as_bytes() != file.hash().as_bytes() {
        return Err(format!("fixture asset BLAKE3 mismatch: {}", file.name()));
    }
    Ok(())
}

/// Verifies only the two fixed fixture atlas names, using bounded file reads.
/// Staging must verify its copied snapshot before assigning canonical asset identities.
fn verify_assets(directory: &Path) -> Result<Value, String> {
    let manifest =
        AssetPackManifest::from_toml(fixture::MANIFEST).map_err(|error| error.to_string())?;
    let mut verified = Vec::new();
    for file in manifest.files() {
        let source_file = match file.density().map(tabula_assets::AssetDensity::get) {
            Some(1) => "tiles@1x.png",
            Some(2) => "tiles@2x.png",
            _ => {
                return Err(String::from(
                    "asset verifier is scoped to the two pinned fixture atlases",
                ))
            }
        };
        let input = std::fs::File::open(directory.join(source_file))
            .map_err(|error| format!("cannot open {source_file}: {error}"))?;
        if input.metadata().map_err(|error| error.to_string())?.len() != file.bytes().get() {
            return Err(format!("fixture asset byte size mismatch: {}", file.name()));
        }
        let mut bytes = Vec::new();
        input
            .take(file.bytes().get().saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|error| format!("cannot read {source_file}: {error}"))?;
        verify_atlas_bytes(file, &bytes)?;
        verified.push(json!({"source_file":source_file,"name":file.name().as_str(),"path":file.path().as_str(),
            "hash":file.hash().to_string(),"bytes":file.bytes().get(),"density":file.density().map(tabula_assets::AssetDensity::get)}));
    }
    if verified.len() != 2 {
        return Err(String::from(
            "asset verifier expects exactly two pinned fixture atlases",
        ));
    }
    Ok(json!({"status":"PASS","pack":manifest.pack_ref().to_string(),"files":verified}))
}

fn themes() -> Value {
    let tokens = [
        TextStyleToken::DisplayLg,
        TextStyleToken::DisplayMd,
        TextStyleToken::DisplaySm,
        TextStyleToken::HeadlineLg,
        TextStyleToken::HeadlineMd,
        TextStyleToken::HeadlineSm,
        TextStyleToken::TitleLg,
        TextStyleToken::TitleMd,
        TextStyleToken::TitleSm,
        TextStyleToken::BodyLg,
        TextStyleToken::BodyMd,
        TextStyleToken::BodySm,
        TextStyleToken::LabelLg,
        TextStyleToken::LabelMd,
        TextStyleToken::LabelSm,
        TextStyleToken::MonoMd,
        TextStyleToken::MonoSm,
    ];
    let mut result = Map::new();
    for scheme in [Scheme::Light, Scheme::Dark, Scheme::HcLight, Scheme::HcDark] {
        let theme = scheme.theme();
        let styles: Map<String,Value> = tokens.iter().map(|token| { let style=theme.text_style(*token);
            (format!("{token:?}"),json!({"font_size":style.size().get(),"line_height":style.line_height().get(),
            "weight":style.weight().get(),"letter_spacing":style.letter_spacing().get(),"family":format!("{:?}",style.family()),
            "monospace":style.tabular_figures()})) }).collect();
        result.insert(
            scheme.name().to_owned(),
            json!({"background":color(theme.color.surface),"text_styles":styles}),
        );
    }
    Value::Object(result)
}

fn export(directory: &Path, full_stream: bool) -> Result<(), String> {
    std::fs::create_dir_all(directory).map_err(|error| error.to_string())?;
    let mut scenarios = Vec::new();
    for (scheme, reduced) in [
        (Scheme::Light, false),
        (Scheme::Dark, false),
        (Scheme::HcLight, false),
        (Scheme::HcDark, false),
        (Scheme::Light, true),
    ] {
        let mut session = Session::new(
            1,
            frame([900.0, 720.0], 1.0, 0, scheme)?,
            scheme,
            reduced,
            Scenario::Scripted,
        )?;
        // Issue-59 samples after its 3s warmup: initial placement motion has settled.
        session.update_frame(frame([900.0, 720.0], 1.0, 3_000, scheme)?, scheme, reduced)?;
        let mut frames = vec![session.frame_json()];
        for step in 0..SCRIPTED {
            let at = 3_750 + u64::from(step) * 750;
            for time in [at - 300, at - 160, at, at + 80, at + 160, at + 300] {
                session.update_frame(frame([900.0, 720.0], 1.0, time, scheme)?, scheme, reduced)?;
                if full_stream {
                    frames.push(session.frame_json());
                }
            }
        }
        if checkpoint(&session.match_) != FINAL_CHECKPOINT {
            return Err(String::from("issue-59 final checkpoint mismatch"));
        }
        if !full_stream {
            frames.push(session.frame_json());
        }
        scenarios.push(json!({"name":if reduced {"light-reduced"} else {scheme.name()},"theme":scheme.name(),"reduced_motion":reduced,
            "motion_samples_exercised":120,"export_full_stream":full_stream,"frames":frames}));
    }
    let revision = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map_or_else(
            || String::from("UNRECORDED"),
            |value| value.trim().to_owned(),
        );
    let data = json!({"schema_version":1,"source_revision":revision,
        "source_revision_scope":"repository base; compile-time source hashes bind this tooling build",
        "source_build_blake3":blake3::hash(include_bytes!("embedding_fixture.rs")).to_hex().to_string(),
        "presenter_source_blake3":blake3::hash(include_bytes!("../../games/tiles/src/presentation.rs")).to_hex().to_string(),
        "rules_source_hash":blake3::Hash::from(TilesModule::rules_hash()).to_hex().to_string(),
        "viewport":[900,720],"dpi":1,
        "initial_inputs":INITIAL,"scripted_inputs":SCRIPTED,"baseline_checkpoint":INITIAL_CHECKPOINT,"final_checkpoint":FINAL_CHECKPOINT,
        "assets":assets()?,"themes":themes(),"scenarios":scenarios,"boundary":"permitted View to RenderList; native Rust authority"});
    for (name, value) in [("fixture.json", data), ("render-contract.json", contract())] {
        std::fs::write(
            directory.join(name),
            serde_json::to_vec(&value).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn main() -> Result<(), String> {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    match arguments.as_slice() {
        [mode, directory] if mode == "--export" => export(Path::new(directory), false),
        [mode, directory] if mode == "--export-full" => export(Path::new(directory), true),
        [mode, directory] if mode == "--verify-assets" => {
            println!("{}", verify_assets(Path::new(directory))?);
            Ok(())
        }
        [mode] if mode == "--stdio" => {
            let mut authority = Authority::default();
            let mut input = std::io::stdin().lock();
            while let Some(line) = bounded_line(&mut input).map_err(|error| error.to_string())? {
                let result = line.and_then(|line| {
                    let request = serde_json::from_str(&line)
                        .map_err(|_| String::from("invalid strict request schema"))?;
                    authority.execute(request)
                });
                let value = result.unwrap_or_else(|error| json!({"ok":false,"error":error}));
                println!("{value}");
            }
            Ok(())
        }
        _ => Err(String::from(
            "usage: embedding_fixture --export DIR | --export-full DIR | --verify-assets DIR | --stdio",
        )),
    }
}

/// Bound allocation before parsing, and drain a hostile line without treating its tail as a request.
fn bounded_line(input: &mut impl BufRead) -> std::io::Result<Option<Result<String, String>>> {
    let mut bytes = Vec::new();
    let mut oversized = false;
    loop {
        let chunk = input.fill_buf()?;
        if chunk.is_empty() {
            if bytes.is_empty() && !oversized {
                return Ok(None);
            }
            break;
        }
        let newline = chunk.iter().position(|byte| *byte == b'\n');
        let length = newline.unwrap_or(chunk.len());
        if !oversized {
            if bytes.len() + length > 16_384 {
                oversized = true;
                bytes.clear();
            } else {
                bytes.extend_from_slice(&chunk[..length]);
            }
        }
        input.consume(length + usize::from(newline.is_some()));
        if newline.is_some() {
            break;
        }
    }
    Ok(Some(if oversized {
        Err(String::from("request line exceeds fixture bound"))
    } else {
        String::from_utf8(bytes).map_err(|_| String::from("request must be UTF-8"))
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn init(scenario: &str, generation: u64) -> Request {
        serde_json::from_value(json!({"op":"init","session_id":"test","generation":generation,"revision":0,
            "viewport":[900,720],"dpi":1,"theme":"light","reduced_motion":false,"scenario":scenario})).unwrap()
    }
    fn input(revision: u64, key: &str, pressed: bool) -> Request {
        serde_json::from_value(json!({"op":"input","session_id":"test","generation":1,"revision":revision,"now_ms":1000,
            "input":{"kind":"key","key":key,"pressed":pressed}})).unwrap()
    }

    #[test]
    fn staging_rejects_same_size_modified_atlas_before_assigning_its_identity() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("tiles@1x.png"), fixture::ATLAS_1X).unwrap();
        std::fs::write(directory.path().join("tiles@2x.png"), fixture::ATLAS_2X).unwrap();
        assert_eq!(
            verify_assets(directory.path()).unwrap()["files"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        let mut modified = fixture::ATLAS_1X.to_vec();
        modified[100] ^= 1;
        assert_eq!(modified.len(), fixture::ATLAS_1X.len());
        std::fs::write(directory.path().join("tiles@1x.png"), &modified).unwrap();
        let error = verify_assets(directory.path()).unwrap_err();
        assert!(
            error.contains("BLAKE3 mismatch"),
            "same-size corruption must fail digest verification: {error}"
        );
        std::fs::write(directory.path().join("tiles@1x.png"), fixture::ATLAS_1X).unwrap();
        std::fs::write(directory.path().join("tiles@2x.png"), [0; 10]).unwrap();
        assert!(verify_assets(directory.path())
            .unwrap_err()
            .contains("byte size mismatch"));
    }

    #[test]
    fn issue59_checkpoints_and_generated_contract_cover_actual_commands() {
        let directory = tempfile::tempdir().unwrap();
        export(directory.path(), true).unwrap();
        let value: Value =
            serde_json::from_slice(&std::fs::read(directory.path().join("fixture.json")).unwrap())
                .unwrap();
        for scenario in value["scenarios"].as_array().unwrap() {
            let frames = scenario["frames"].as_array().unwrap();
            assert_eq!(frames.first().unwrap()["checkpoint"], INITIAL_CHECKPOINT);
            assert_eq!(frames.last().unwrap()["checkpoint"], FINAL_CHECKPOINT);
            assert_eq!(frames.last().unwrap()["input_count"], 44);
            for frame in frames {
                for command in frame["commands"].as_array().unwrap() {
                    let mut keys = command
                        .as_object()
                        .unwrap()
                        .keys()
                        .map(String::as_str)
                        .collect::<Vec<_>>();
                    keys.sort_unstable();
                    let mut expected = command_fields(command["kind"].as_str().unwrap())
                        .unwrap()
                        .to_vec();
                    expected.sort_unstable();
                    assert_eq!(keys, expected);
                }
            }
        }
        assert_eq!(value["assets"]["files"].as_array().unwrap().len(), 2);
        let serialized = serde_json::to_string(&value).unwrap();
        for forbidden in [
            "remaining_order",
            "rng_seed",
            "\"seed\"",
            "\"state\"",
            "\"bag\"",
        ] {
            assert!(
                !serialized.contains(forbidden),
                "export discloses {forbidden}"
            );
        }
    }

    #[test]
    fn generations_and_revisions_reject_late_and_duplicate_operations() {
        let mut authority = Authority::default();
        authority.execute(init("interactive", 1)).unwrap();
        assert!(authority.execute(init("interactive", 1)).is_err());
        authority.execute(input(0, "Space", true)).unwrap();
        assert!(authority.execute(input(0, "Enter", true)).is_err());
        assert_eq!(
            checkpoint(&authority.sessions["test"].match_),
            INITIAL_CHECKPOINT
        );
        let dispose = serde_json::from_value(
            json!({"op":"dispose","session_id":"test","generation":1,"revision":1}),
        )
        .unwrap();
        authority.execute(dispose).unwrap();
        assert!(authority.execute(input(1, "Enter", true)).is_err());
        assert!(authority.execute(init("interactive", 1)).is_err());
        authority.execute(init("interactive", 2)).unwrap();
        assert!(authority.execute(input(0, "Enter", true)).is_err());
    }

    #[test]
    fn strict_input_geometry_and_schema_fail_without_transition() {
        let mut authority = Authority::default();
        authority.execute(init("interactive", 1)).unwrap();
        for invalid in [
            json!({"kind":"key","key":"Delete","pressed":true}),
            json!({"kind":"key","key":"Enter","pressed":true,"command":"SkipMeeple"}),
        ] {
            assert!(serde_json::from_value::<RawInput>(invalid).is_err());
        }
        let invalid = serde_json::from_value(
            json!({"op":"input","session_id":"test","generation":1,"revision":0,"now_ms":10,
            "input":{"kind":"pointer","position":[1e30,0],"button":"primary","phase":"up"}}),
        )
        .unwrap();
        assert!(authority.execute(invalid).is_err());
        assert_eq!(authority.sessions["test"].revision, 0);
        assert_eq!(
            checkpoint(&authority.sessions["test"].match_),
            INITIAL_CHECKPOINT
        );
        assert!(frame([0.0, 720.0], 1.0, 0, Scheme::Light).is_err());
        assert!(frame([900.0, 720.0], f32::NAN, 0, Scheme::Light).is_err());
        let request = json!({"op":"frame","session_id":"test","generation":1,"revision":0,"now_ms":0,
            "viewport":[900,720],"dpi":1,"theme":"light","reduced_motion":false,"state":{"bag":[1]}});
        assert!(serde_json::from_value::<Request>(request).is_err());
        assert!(serde_json::from_value::<RawInput>(
            json!({"kind":"focus","focused":false,"state":{}})
        )
        .is_err());
        for viewport in [[199.0, 720.0], [4097.0, 720.0], [900.0, 199.0]] {
            assert!(frame(viewport, 1.0, 0, Scheme::Light).is_err());
        }
        for dpi in [0.0, 0.49, 4.01, f32::INFINITY] {
            assert!(frame([900.0, 720.0], dpi, 0, Scheme::Light).is_err());
        }
        assert!(validate_identity("test", MAX_SAFE_INTEGER + 1, 0).is_err());
        assert!(validate_identity("test", 1, MAX_SAFE_INTEGER + 1).is_err());
    }

    #[test]
    fn request_reader_bounds_allocation_and_drains_oversized_or_invalid_utf8_lines() {
        let mut bytes = vec![b'x'; 20_000];
        bytes.extend_from_slice(b"\n{\"op\":\"dispose\"}\n\xff\nvalid\n");
        let mut input = std::io::BufReader::with_capacity(7, std::io::Cursor::new(bytes));
        assert!(bounded_line(&mut input).unwrap().unwrap().is_err());
        assert_eq!(
            bounded_line(&mut input).unwrap().unwrap().unwrap(),
            "{\"op\":\"dispose\"}"
        );
        assert!(bounded_line(&mut input).unwrap().unwrap().is_err());
        assert_eq!(bounded_line(&mut input).unwrap().unwrap().unwrap(), "valid");
        assert!(bounded_line(&mut input).unwrap().is_none());
    }

    #[test]
    fn compact_fixture_retains_settled_initial_and_final_frames() {
        let directory = tempfile::tempdir().unwrap();
        export(directory.path(), false).unwrap();
        let fixture: Value =
            serde_json::from_slice(&std::fs::read(directory.path().join("fixture.json")).unwrap())
                .unwrap();
        for scenario in fixture["scenarios"].as_array().unwrap() {
            assert_eq!(scenario["frames"].as_array().unwrap().len(), 2);
            assert_eq!(scenario["frames"][0]["at_ms"], 3000);
            assert_eq!(scenario["frames"][0]["checkpoint"], INITIAL_CHECKPOINT);
            assert_eq!(scenario["frames"][1]["checkpoint"], FINAL_CHECKPOINT);
        }
    }

    #[test]
    fn generic_presenter_rejection_preserves_checkpoint_and_focus_cancels_activation() {
        let mut authority = Authority::default();
        authority.execute(init("interactive", 1)).unwrap();
        // A hostile typed command reaches the ordinary reducer but is invalid in this phase.
        let session = authority.sessions.get_mut("test").unwrap();
        let rejected = session.match_.submit_input(
            tabula_game_api::Input::Player {
                seat: session.match_.view().turn,
                command: tabula_game_tiles::rules::Command::SkipMeeple,
            },
            &session.frame,
        );
        assert!(matches!(rejected, Err(LocalMatchError::Rejected(_))));
        assert_eq!(checkpoint(&session.match_), INITIAL_CHECKPOINT);
        assert_eq!(session.match_.replay_trace().accepted_inputs().len(), 24);
        // The presenter independently suppresses an occupied-square Enter before rules.
        let result = authority.execute(input(0, "Enter", true)).unwrap();
        assert_eq!(result["outcome"], "local");
        assert_eq!(
            result["envelope"]["frame"]["checkpoint"],
            INITIAL_CHECKPOINT
        );
        assert_eq!(result["envelope"]["frame"]["input_count"], 24);
        let blur = serde_json::from_value(
            json!({"op":"input","session_id":"test","generation":1,"revision":1,"now_ms":1000,
            "input":{"kind":"focus","focused":false}}),
        )
        .unwrap();
        authority.execute(blur).unwrap();
        let result = authority.execute(input(2, "Enter", true)).unwrap();
        assert_eq!(result["outcome"], "local");
        assert_eq!(result["envelope"]["frame"]["input_count"], 24);
    }

    #[test]
    fn generic_keyboard_intent_reaches_rust_rules_and_changes_checkpoint() {
        let mut authority = Authority::default();
        authority.execute(init("interactive", 1)).unwrap();
        authority.execute(input(0, "Tab", true)).unwrap();
        let result = authority.execute(input(1, "Enter", true)).unwrap();
        assert_eq!(result["outcome"], "accepted");
        assert_eq!(result["envelope"]["frame"]["input_count"], 25);
        assert_ne!(
            result["envelope"]["frame"]["checkpoint"],
            INITIAL_CHECKPOINT
        );
    }
}
