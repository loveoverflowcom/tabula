//! D05 presentation pilot: private per-age runtime packs through the existing
//! verified-asset, `RenderList` and Macroquad renderer path. // xtask-allow-game-id: Age War harness
//!
//! This is an animation/design harness, not gameplay. It implements no combat
//! rule, target choice, damage, cooldown or outcome; actors follow an authored
//! storyboard and markers spawn non-authoritative cues (doc 00 I-10/I-12).
//! Inputs are owner-private packs built by
//! `games/age-war/design-tools/runtime/build_runtime_packs.py` and sealed by
//! `cargo xtask pack-assets --external`; nothing private is embedded here.
//!
//! ```text
//! check --packs DIR [--ages A,B] [--report FILE]          no window; integrity + D01 oracle
//! hud-catalog [--report FILE]                              public D01 facts for the D06 preview
//! run   --packs DIR --age AGE [--scenario lineup|crowd|loadcycle] [--dpi F]
//!       [--speed 0.5|1|2] [--effects full|low|reduced] [--crowd N] [--size WxH]
//!       [--fixed-dt MS] [--frames N] [--capture-dir DIR --capture-at MS,..]
//!       [--budget-mib N] [--audio] [--report FILE]
//! ```

#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::float_arithmetic,
    clippy::too_many_lines
)]

mod check;
mod clips;
mod hud_catalog;
mod load;
mod scene;
mod sound;

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write as _,
    path::PathBuf,
};

use glam::Vec2;
use macroquad::prelude as mq;
use tabula_design::{Theme, ThemeKind};
use tabula_game_client::{host_resources::load_builtin_fonts, resolve_display_geometry};
use tabula_presentation::{Camera2D, RenderListBuilder, Renderer};
use tabula_render_macroquad::{
    assets::{AssetCacheLimits, MacroquadTextureUploader, SpriteAssetCache},
    density_for_dpi, MacroquadRenderer,
};

use crate::{
    clips::{ClipSet, Facing},
    load::{prepare, process_memory, LoadStats, Pack},
    scene::{Effects, Kind, Stage},
};

const AGES: [&str; 6] = [
    "Primitive",
    "Ancient",
    "Feudal",
    "Arcane",
    "Industrial",
    "Future",
];

#[derive(Clone, Debug)]
struct Options {
    mode: String,
    packs: PathBuf,
    version: String,
    ages: Vec<String>,
    scenario: String,
    dpi: f32,
    speed: f32,
    effects: Effects,
    crowd: usize,
    size: (i32, i32),
    fixed_dt: Option<f32>,
    frames: Option<u64>,
    capture_dir: Option<PathBuf>,
    capture_at: Vec<f32>,
    budget_mib: u64,
    audio: bool,
    audio_volume: f32,
    report: Option<PathBuf>,
}

fn parse_options() -> Result<Options, String> {
    let mut args = std::env::args().skip(1);
    let mode = args
        .next()
        .ok_or("usage: age_war_d05_pilot check|run ...")?;
    let mut options = Options {
        mode,
        packs: PathBuf::from("target/age-war-d05/packs"), // xtask-allow-game-id: explicit example fixture path
        version: "0.5.0".into(),
        ages: AGES.iter().map(ToString::to_string).collect(),
        scenario: "lineup".into(),
        dpi: 1.0,
        speed: 1.0,
        effects: Effects::Full,
        crowd: 25,
        size: (1280, 720),
        fixed_dt: None,
        frames: None,
        capture_dir: None,
        capture_at: Vec::new(),
        budget_mib: 256,
        audio: false,
        audio_volume: 0.8,
        report: None,
    };
    while let Some(flag) = args.next() {
        if flag == "--audio" {
            options.audio = true;
            continue;
        }
        let value = args.next().ok_or_else(|| format!("{flag} needs a value"))?;
        let bad = |what: &str| format!("{flag}: invalid {what} {value:?}");
        match flag.as_str() {
            "--packs" => options.packs = value.clone().into(),
            "--version" => options.version.clone_from(&value),
            "--ages" | "--age" => {
                options.ages = value.split(',').map(str::to_string).collect();
                if options.ages.iter().any(|a| !AGES.contains(&a.as_str())) {
                    return Err(bad("age"));
                }
            }
            "--scenario" => options.scenario.clone_from(&value),
            "--dpi" => {
                options.dpi = value
                    .parse()
                    .ok()
                    .filter(|d: &f32| (1.0..=4.0).contains(d))
                    .ok_or_else(|| bad("dpi"))?;
            }
            "--speed" => {
                options.speed = value
                    .parse()
                    .ok()
                    .filter(|s| [0.5, 1.0, 2.0].contains(s))
                    .ok_or_else(|| bad("speed"))?;
            }
            "--effects" => {
                options.effects = match value.as_str() {
                    "full" => Effects::Full,
                    "low" => Effects::Low,
                    "reduced" => Effects::Reduced,
                    _ => return Err(bad("effects")),
                }
            }
            "--crowd" => {
                options.crowd = value
                    .parse()
                    .ok()
                    .filter(|n| (1..=200).contains(n))
                    .ok_or_else(|| bad("count"))?;
            }
            "--size" => {
                let (w, h) = value.split_once('x').ok_or_else(|| bad("size"))?;
                options.size = (
                    w.parse().map_err(|_| bad("size"))?,
                    h.parse().map_err(|_| bad("size"))?,
                );
            }
            "--fixed-dt" => options.fixed_dt = Some(value.parse().map_err(|_| bad("dt"))?),
            "--frames" => options.frames = Some(value.parse().map_err(|_| bad("frames"))?),
            "--capture-dir" => options.capture_dir = Some(value.clone().into()),
            "--capture-at" => {
                options.capture_at = value
                    .split(',')
                    .map(str::parse)
                    .collect::<Result<_, _>>()
                    .map_err(|_| bad("times"))?;
            }
            "--audio-volume" => {
                options.audio_volume = value
                    .parse()
                    .ok()
                    .filter(|v| (0.0..=1.0).contains(v))
                    .ok_or_else(|| bad("volume"))?;
            }
            "--budget-mib" => options.budget_mib = value.parse().map_err(|_| bad("budget"))?,
            "--report" => options.report = Some(value.clone().into()),
            _ => return Err(format!("unknown flag {flag}")),
        }
    }
    Ok(options)
}

fn json_str(value: &str) -> String {
    let mut out = String::from("\"");
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn emit(options: &Options, json: &str) -> Result<(), String> {
    println!("AGE_WAR_D05_PILOT {json}");
    if let Some(path) = &options.report {
        std::fs::write(path, format!("{json}\n"))
            .map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(())
}

fn pack_name(age: &str) -> String {
    format!("age-war-{}", age.to_lowercase()) // xtask-allow-game-id: this example's private fixture namespace
}

/// No window: verify every pack file, resolve every resource at both densities,
/// parse sidecars and cross-check delivered timing against the D01 catalog.
fn run_check(options: &Options) -> Result<(), String> {
    let mut ages_json = Vec::new();
    let mut total_failures = 0;
    for age in &options.ages {
        let started = std::time::Instant::now();
        let pack = Pack::open(&options.packs, &pack_name(age), &options.version)?;
        let parse_ms = started.elapsed().as_secs_f64() * 1000.0;
        let resolve_started = std::time::Instant::now();
        let mut resolved = 0_usize;
        for resource in pack.manifest.resources() {
            for density in [1, 2] {
                pack.file_for(resource.id().as_str(), density)?;
                resolved += 1;
            }
        }
        let resolve_ms = resolve_started.elapsed().as_secs_f64() * 1000.0;
        let verify_started = std::time::Instant::now();
        let mut bytes = 0_u64;
        for file in pack.manifest.files() {
            bytes += pack.verified(file)?.bytes().len() as u64;
        }
        let verify_ms = verify_started.elapsed().as_secs_f64() * 1000.0;
        let set = ClipSet::parse(&pack.text("meta/clips")?)?;
        let vfx = scene::parse_vfx(&pack.text("meta/vfx")?)?;
        let mut report = check::CheckReport::default();
        check::check_age(&set, &mut report);
        total_failures += report.failures();
        let passed: Vec<String> = report
            .passed
            .iter()
            .map(|(k, v)| format!("{}:{v}", json_str(k)))
            .collect();
        let failed: Vec<String> = report
            .failed
            .iter()
            .map(|(k, v)| {
                format!(
                    "{}:[{}]",
                    json_str(k),
                    v.iter()
                        .take(8)
                        .map(|d| json_str(d))
                        .collect::<Vec<_>>()
                        .join(",")
                )
            })
            .collect();
        ages_json.push(format!(
            "{{\"age\":{},\"files\":{},\"verified_bytes\":{bytes},\"manifest_bytes\":{},\"resources\":{},\"resolutions\":{resolved},\"parse_ms\":{parse_ms:.1},\"resolve_ms\":{resolve_ms:.1},\"verify_ms\":{verify_ms:.1},\"clips\":{},\"vfx_tracks\":{},\"passed\":{{{}}},\"failed\":{{{}}}}}",
            json_str(age), pack.manifest.files().len(), pack.manifest_bytes, pack.manifest.resources().len(),
            set.clips.len(), vfx.len(), passed.join(","), failed.join(",")
        ));
    }
    let common = Pack::open(&options.packs, "age-war-common", &options.version)?; // xtask-allow-game-id: explicit example fixture
    let mut common_bytes = 0_u64;
    for file in common.manifest.files() {
        common_bytes += common.verified(file)?.bytes().len() as u64;
    }
    emit(options, &format!(
        "{{\"kind\":\"check\",\"authority\":false,\"ages\":[{}],\"common\":{{\"files\":{},\"verified_bytes\":{common_bytes}}},\"d01_failures\":{total_failures}}}",
        ages_json.join(","), common.manifest.files().len()
    ))?;
    if total_failures > 0 {
        return Err(format!("{total_failures} D01 timing/coverage mismatches"));
    }
    Ok(())
}

/// Logical viewport and DPI as the renderer sees them. `--dpi` emulates a
/// high-density display: the renderer then selects its own asset density
/// with `density_for_dpi`, exactly as on device.
fn display(options: &Options) -> Option<(tabula_presentation::Viewport, tabula_presentation::Dpi)> {
    resolve_display_geometry(
        mq::screen_width() / options.dpi,
        mq::screen_height() / options.dpi,
        options.dpi,
    )
}

/// Density the renderer will request for this DPI; packs resolve the nearest.
fn load_density(options: &Options) -> u8 {
    tabula_presentation::Dpi::new(options.dpi).map_or(1, |dpi| density_for_dpi(dpi).get())
}

/// D01 half-widths in stage units: 10,000 q span the 1,920-unit focus stage.
fn footprints() -> BTreeMap<String, f32> {
    tabula_game_age_war::catalog::UNITS // xtask-allow-game-id: harness-only footprint oracle
        .iter()
        .map(|unit| {
            (
                unit.id.as_str().to_string(),
                unit.half_width_q.get() as f32 * 0.192,
            )
        })
        .collect()
}

fn now() -> f64 {
    mq::get_time()
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    sorted[((sorted.len() - 1) as f64 * p).round() as usize]
}

struct AgeLoad {
    pack: Pack,
    set: ClipSet,
    vfx: scene::VfxIndex,
    /// Commander-spell VFX recipes: packed recipes no unit references.
    spells: Vec<String>,
    stats: LoadStats,
    parse_ms: f64,
}

/// Binds an age pack and prepares only the unit-facing groups and VFX needed.
fn load_age(
    options: &Options,
    renderer: &mut MacroquadRenderer,
    age: &str,
    groups: &BTreeSet<(String, Facing)>,
    recipes: &BTreeSet<String>,
) -> Result<AgeLoad, String> {
    let started = now();
    let pack = Pack::open(&options.packs, &pack_name(age), &options.version)?;
    renderer
        .assets_mut()
        .bind_pack(&pack.manifest, &pack.game, &pack.reference)
        .map_err(|e| format!("bind {age}: {e}"))?;
    let set = ClipSet::parse(&pack.text("meta/clips")?)?;
    let vfx = scene::parse_vfx(&pack.text("meta/vfx")?)?;
    let parse_ms = (now() - started) * 1000.0;
    let mut files = vec![pack.file_for("scene/backdrop", 1)?];
    let wanted: Vec<(String, Facing)> = if groups.is_empty() {
        set.units
            .iter()
            .flat_map(|u| [(u.id.clone(), Facing::Right), (u.id.clone(), Facing::Left)])
            .collect()
    } else {
        groups.iter().cloned().collect()
    };
    for (unit, facing) in &wanted {
        for kind in ["unit", "mask"] {
            for file in pack.files_for_prefix(
                &format!("{kind}/{unit}/{}/", facing.as_str()),
                load_density(options),
            )? {
                if !files.contains(&file) {
                    files.push(file);
                }
            }
        }
    }
    let recipe_set: BTreeSet<String> = if recipes.is_empty() {
        set.units
            .iter()
            .flat_map(|u| {
                [u.vfx_weapon.clone(), u.vfx_impact.clone()]
                    .into_iter()
                    .chain(u.skills.clone())
            })
            .collect()
    } else {
        recipes.clone()
    };
    for recipe in &recipe_set {
        for file in pack.files_for_prefix(&format!("vfx/{recipe}/"), 1)? {
            if !files.contains(&file) {
                files.push(file);
            }
        }
    }
    let unit_recipes: BTreeSet<String> = set
        .units
        .iter()
        .flat_map(|u| {
            [u.vfx_weapon.clone(), u.vfx_impact.clone()]
                .into_iter()
                .chain(u.skills.clone())
        })
        .collect();
    let spells: Vec<String> = vfx
        .keys()
        .map(|(recipe, _, _)| recipe.clone())
        .filter(|recipe| {
            !unit_recipes.contains(recipe)
                && !recipe.starts_with("drone_")
                && !recipe.starts_with("impact_")
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    for spell in &spells {
        for file in pack.files_for_prefix(&format!("vfx/{spell}/"), 1)? {
            if !files.contains(&file) {
                files.push(file);
            }
        }
    }
    let stats = prepare(&pack, renderer.assets_mut(), &files, now)?;
    Ok(AgeLoad {
        pack,
        set,
        vfx,
        spells,
        stats,
        parse_ms,
    })
}

fn memory_json() -> String {
    let memory = process_memory();
    format!(
        "{{\"rss_kib\":{},\"peak_kib\":{}}}",
        memory.rss_kib, memory.peak_kib
    )
}

fn cache_json(renderer: &MacroquadRenderer) -> String {
    let stats = renderer.assets().stats();
    format!(
        "{{\"resident_rgba_bytes\":{},\"resident_textures\":{},\"decodes\":{},\"uploads\":{},\"releases\":{}}}",
        stats.resident_bytes, stats.resident_textures, stats.decodes, stats.uploads, stats.releases
    )
}

/// Runs frames until the requested count/time, capturing and measuring.
#[allow(clippy::too_many_arguments)]
async fn play(
    options: &Options,
    renderer: &mut MacroquadRenderer,
    loaded: &AgeLoad,
    actors: &[scene::Actor],
    frames: u64,
    label: &str,
    captures: &mut Vec<String>,
    cue_log: &mut Vec<String>,
    mut mixer: Option<&mut sound::Mixer>,
) -> Result<String, String> {
    let theme = Theme::by_kind(ThemeKind::Light);
    let started = now();
    let mut previous_t = -1.0_f32;
    let mut cues: Vec<scene::Cue> = Vec::new();
    let mut cpu = Vec::new();
    let mut intervals = Vec::new();
    let mut last_frame = started;
    let mut capture_queue: Vec<f32> = options.capture_at.clone();
    let mut sprites_max = 0;
    let mut cue_counts: BTreeMap<&'static str, u64> = BTreeMap::new();
    let cue_cap = match options.effects {
        Effects::Full => 96,
        Effects::Low => 24,
        Effects::Reduced => 48,
    };
    for index in 0..frames {
        let frame_start = now();
        intervals.push((frame_start - last_frame) * 1000.0);
        last_frame = frame_start;
        let wall_ms = options
            .fixed_dt
            .map_or((frame_start - started) as f32 * 1000.0, |dt| {
                index as f32 * dt
            });
        let t = wall_ms * options.speed;
        let Some((viewport, dpi)) = display(options) else {
            mq::next_frame().await;
            continue;
        };
        let mut fresh = Vec::new();
        for actor in actors {
            actor.cues(&loaded.set, previous_t, t, &mut fresh);
        }
        scene::spell_cues(&loaded.spells, previous_t, t, &mut fresh);
        for cue in &fresh {
            if let Some(mixer) = mixer.as_deref_mut() {
                mixer.cue(cue, t);
            }
            *cue_counts.entry(cue.kind).or_default() += 1;
            cue_log.push(format!(
                "{t:.1}\t{}\t{}\t{}\t{}",
                cue.kind, cue.recipe, cue.unit, cue.team
            ));
        }
        cues.extend(fresh);
        cues.retain(|cue| {
            t - cue.born_ms <= scene::cue_lifetime(&loaded.vfx, cue, options.effects)
        });
        if cues.len() > cue_cap {
            let excess = cues.len() - cue_cap;
            cues.drain(..excess);
        }
        previous_t = t;
        let frame = renderer.begin_frame(viewport, dpi, t as u64, theme);
        let mut builder = RenderListBuilder::new(Camera2D::default());
        let stage = Stage::fit(Vec2::new(viewport.size().x, viewport.size().y));
        let text = format!("{label} · t={t:.0}ms · animation/design pilot, not gameplay");
        let sprites = scene::draw(
            &mut builder,
            &frame.theme(),
            &loaded.set,
            stage,
            actors,
            t,
            &cues,
            &loaded.vfx,
            options.effects,
            &text,
        )?;
        sprites_max = sprites_max.max(sprites);
        let list = builder.finish().map_err(|e| format!("{e:?}"))?;
        renderer
            .submit(&list)
            .map_err(|e| format!("submit: {e:?}"))?;
        renderer
            .end_frame()
            .map_err(|e| format!("end_frame: {e:?}"))?;
        cpu.push((now() - frame_start) * 1000.0);
        if let Some(dir) = &options.capture_dir {
            while capture_queue.first().is_some_and(|at| t >= *at) {
                let at = capture_queue.remove(0);
                let path =
                    dir.join(format!("{label}-t{at:05.0}.png").replace([' ', '·', '/'], "_"));
                mq::get_screen_data().export_png(&path.to_string_lossy());
                captures.push(path.to_string_lossy().into_owned());
            }
        }
        mq::next_frame().await;
    }
    intervals.remove(0);
    cpu.sort_by(f64::total_cmp);
    intervals.sort_by(f64::total_cmp);
    let counts: Vec<String> = cue_counts
        .iter()
        .map(|(k, v)| format!("{}:{v}", json_str(k)))
        .collect();
    Ok(format!(
        "{{\"label\":{},\"frames\":{frames},\"actors\":{},\"max_sprites\":{sprites_max},\"cpu_ms\":{{\"p50\":{:.3},\"p95\":{:.3},\"max\":{:.3}}},\"interval_ms\":{{\"p50\":{:.3},\"p95\":{:.3},\"max\":{:.3}}},\"cues\":{{{}}}}}",
        json_str(label), actors.len(), percentile(&cpu, 0.5), percentile(&cpu, 0.95), percentile(&cpu, 1.0),
        percentile(&intervals, 0.5), percentile(&intervals, 0.95), percentile(&intervals, 1.0), counts.join(",")
    ))
}

async fn flush(options: &Options, renderer: &mut MacroquadRenderer, frames: usize) {
    let theme = Theme::by_kind(ThemeKind::Light);
    for _ in 0..frames {
        if let Some((viewport, dpi)) = display(options) {
            let _ = renderer.begin_frame(viewport, dpi, 0, theme);
            let _ = renderer.end_frame();
        }
        mq::next_frame().await;
    }
    renderer.assets_mut().collect_released();
}

async fn run_window(options: Options) -> Result<(), String> {
    let mut renderer = MacroquadRenderer::new();
    load_builtin_fonts(&mut renderer).await?;
    let budget = options.budget_mib * 1024 * 1024;
    *renderer.assets_mut() = SpriteAssetCache::new(
        MacroquadTextureUploader,
        AssetCacheLimits::new(
            16 * 1024 * 1024,
            4096,
            16 * 1024 * 1024,
            160 * 1024 * 1024,
            budget,
            512,
        )
        .map_err(|e| format!("{e:?}"))?,
    );
    if let Some(dir) = &options.capture_dir {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let baseline = memory_json();
    let mut captures = Vec::new();
    let mut cue_log = Vec::new();
    let mut runs = Vec::new();
    let frames = options.frames.unwrap_or(600);
    let kind = if options.scenario == "crowd" {
        Kind::Crowd
    } else {
        Kind::Lineup
    };
    let sequence: Vec<String> = if options.scenario == "loadcycle" {
        options.ages.clone()
    } else {
        options.ages[..1].to_vec()
    };
    for age in &sequence {
        let before = memory_json();
        // Probe the age's sidecar first so only the groups/recipes the storyboard uses are prepared.
        let probe = Pack::open(&options.packs, &pack_name(age), &options.version)?;
        let probe_set = ClipSet::parse(&probe.text("meta/clips")?)?;
        let actors = scene::actors(&probe_set, kind, options.crowd, &footprints());
        let groups: BTreeSet<(String, Facing)> =
            actors.iter().map(|a| (a.unit.clone(), a.facing)).collect();
        let recipes: BTreeSet<String> = probe_set
            .units
            .iter()
            .filter(|u| groups.iter().any(|g| g.0 == u.id))
            .flat_map(|u| {
                [u.vfx_weapon.clone(), u.vfx_impact.clone()]
                    .into_iter()
                    .chain(u.skills.clone())
            })
            .collect();
        drop(probe);
        let loaded = load_age(&options, &mut renderer, age, &groups, &recipes)?;
        let after_load = memory_json();
        let cache_loaded = cache_json(&renderer);
        let label = format!(
            "{age} {} dpi{} d{} {}x {:?} {}x{}",
            options.scenario,
            options.dpi,
            load_density(&options),
            options.speed,
            options.effects,
            options.size.0,
            options.size.1
        );
        let mut mixer = if options.audio {
            let common = Pack::open(&options.packs, "age-war-common", &options.version)?; // xtask-allow-game-id: explicit example fixture
            let list: Vec<String> = recipes.iter().cloned().collect();
            Some(sound::Mixer::load(&common, &loaded.pack, age, &list, options.audio_volume).await?)
        } else {
            None
        };
        let play_json = play(
            &options,
            &mut renderer,
            &loaded,
            &actors,
            frames,
            &label,
            &mut captures,
            &mut cue_log,
            mixer.as_mut(),
        )
        .await?;
        let audio_json = mixer.as_mut().map_or_else(
            || "null".to_string(),
            |mixer| {
                mixer.stop_loops();
                format!(
                    "{{\"loaded\":{},\"played\":{},\"dropped\":{},\"volume\":{}}}",
                    mixer.loaded, mixer.played, mixer.dropped, options.audio_volume
                )
            },
        );
        let pack_ref = loaded.pack.reference.clone();
        renderer.assets_mut().release_pack(&pack_ref);
        drop(loaded.pack);
        flush(&options, &mut renderer, 2).await;
        let after_release = memory_json();
        runs.push(format!(
            "{{\"age\":{},\"groups\":{},\"recipes\":{},\"files\":{},\"encoded_bytes\":{},\"parse_bind_ms\":{:.1},\"verify_ms\":{:.1},\"decode_upload_ms\":{:.1},\"memory_before\":{before},\"memory_loaded\":{after_load},\"cache_loaded\":{cache_loaded},\"play\":{play_json},\"audio\":{audio_json},\"memory_released\":{after_release},\"cache_released\":{}}}",
            json_str(age), groups.len(), recipes.len(), loaded.stats.files, loaded.stats.encoded_bytes, loaded.parse_ms,
            loaded.stats.verify_ms, loaded.stats.decode_upload_ms, cache_json(&renderer)
        ));
    }
    if let Some(dir) = &options.capture_dir {
        let log = format!(
            "# presentation_ms\tkind\trecipe\tunit\tteam\n{}\n",
            cue_log.join("\n")
        );
        std::fs::write(dir.join("cue-log.tsv"), log).map_err(|e| e.to_string())?;
    }
    let caps: Vec<String> = captures.iter().map(|c| json_str(c)).collect();
    emit(&options, &format!(
        "{{\"kind\":\"run\",\"authority\":false,\"scenario\":{},\"dpi\":{},\"density\":{},\"speed\":{},\"effects\":{},\"size\":[{},{}],\"fixed_dt\":{},\"budget_mib\":{},\"audio\":{},\"memory_baseline\":{baseline},\"runs\":[{}],\"captures\":[{}],\"cues_logged\":{}}}",
        json_str(&options.scenario), options.dpi, load_density(&options), options.speed, json_str(&format!("{:?}", options.effects)),
        options.size.0, options.size.1, options.fixed_dt.map_or("null".into(), |v| v.to_string()), options.budget_mib,
        options.audio, runs.join(","), caps.join(","), cue_log.len()
    ))
}

fn main() {
    let options = match parse_options() {
        Ok(options) => options,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };
    if options.mode == "hud-catalog" {
        let json = hud_catalog::export();
        if let Err(error) = emit(&options, &json) {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }
    if options.mode == "check" {
        if let Err(error) = run_check(&options) {
            eprintln!("check failed: {error}");
            std::process::exit(1);
        }
        return;
    }
    let config = mq::Conf {
        window_title: "Age War D05 presentation pilot".into(),
        window_width: options.size.0,
        window_height: options.size.1,
        high_dpi: false,
        ..Default::default()
    };
    macroquad::Window::from_config(config, async move {
        if let Err(error) = run_window(options).await {
            eprintln!("run failed: {error}");
            std::process::exit(1);
        }
    });
}
