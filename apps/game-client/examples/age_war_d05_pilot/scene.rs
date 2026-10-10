//! Scripted presentation facts for the pilot. Not rules: positions, timing and
//! cues below are an authored storyboard. Nothing computes damage, targets,
//! cooldowns or outcomes, and a marker only spawns a non-authoritative cue.

use std::collections::BTreeMap;

use glam::Vec2;
use tabula_design::{Color, Theme};
use tabula_presentation::{
    Align, AssetRef, Border, Layer, Paint, Rect, RenderCmd, RenderListBuilder, RenderListError,
    TextStyleToken,
};

use crate::clips::{Clip, ClipSet, Facing};

/// D03 stage in logical units (desktop reference scale) and its ground row.
pub const STAGE: Vec2 = Vec2::new(1920.0, 698.0);
pub const GROUND_Y: f32 = 490.0;
/// One delivered VFX texel covers two stage units (authored at mobile 0.5 scale).
const VFX_LU_PER_TEXEL: f32 = 2.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effects {
    Full,
    Low,
    Reduced,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Six units per side through idle, walk, attack, skills, hit and death.
    Lineup,
    /// Many actors trading attack/skill/hit loops at the front.
    Crowd,
}

#[derive(Clone, Debug)]
enum Step {
    Idle(f32),
    Move { clip: String, ms: f32 },
    Clip { clip: String, hold_ms: f32 },
    Fade(f32),
}

impl Step {
    fn duration(&self, set: &ClipSet, unit: &str, facing: Facing) -> f32 {
        match self {
            Self::Idle(ms) | Self::Fade(ms) | Self::Move { ms, .. } => *ms,
            Self::Clip { clip, hold_ms } => {
                set.clip(unit, facing, clip).map_or(0.0, |c| c.duration_ms) + hold_ms
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct Actor {
    pub unit: String,
    pub facing: Facing,
    pub team: usize,
    pub home_x: f32,
    pub row: f32,
    offset_ms: f32,
    steps: Vec<Step>,
    cycle_ms: f32,
    loco_speed: f32,
}

/// A non-authoritative visual cue spawned by a marker or step change.
#[derive(Clone, Debug)]
pub struct Cue {
    pub recipe: String,
    pub team: usize,
    pub phase: &'static str,
    pub born_ms: f32,
    pub at: Vec2,
    pub kind: &'static str,
    pub unit: String,
    /// Ground x for a commander telegraph, which has no casting unit.
    pub ground_x: Option<f32>,
}

/// Illustrative commander storyboard: each age spell telegraphs at a fixed
/// lane point, then shows its contact phase. Timing is a presentation
/// placeholder, not the D01 cast/resolve rule.
pub fn spell_cues(spells: &[String], from_ms: f32, to_ms: f32, out: &mut Vec<Cue>) {
    const PERIOD: f32 = 10_000.0;
    for (slot, spell) in spells.iter().enumerate() {
        let team = slot % 2;
        let cast = 5_000.0 + slot as f32 * 1_500.0;
        let x = if team == 0 { 1_160.0 } else { 760.0 };
        for (offset, phase) in [(0.0, "start"), (900.0, "contact")] {
            let first = ((from_ms - cast - offset) / PERIOD).floor() as i64;
            for cycle in first..=first + 2 {
                let at = cast + offset + cycle as f32 * PERIOD;
                if at > from_ms && at <= to_ms {
                    out.push(Cue {
                        recipe: spell.clone(),
                        team,
                        phase,
                        born_ms: at,
                        at: Vec2::new(0.0, -20.0),
                        kind: "spell",
                        unit: String::new(),
                        ground_x: Some(x),
                    });
                }
            }
        }
    }
}

/// Where an actor is and what it shows at one presentation time.
#[derive(Clone, Debug)]
pub struct Pose<'a> {
    pub clip: &'a Clip,
    pub frame: usize,
    pub x: f32,
    pub opacity: f32,
}

/// One VFX frame record from `meta/vfx.tsv`.
#[derive(Clone, Debug)]
pub struct VfxFrame {
    pub ordinal: usize,
    pub ms: f32,
    pub pivot: Vec2,
    pub size: Vec2,
}

pub type VfxIndex = BTreeMap<(String, String, String), Vec<VfxFrame>>;

pub fn parse_vfx(text: &str) -> Result<VfxIndex, String> {
    let mut index = VfxIndex::new();
    for line in text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
    {
        let f: Vec<&str> = line.split('\t').collect();
        let [_, recipe, team, phase, ordinal, ms, px, py, w, h, _mode] = f.as_slice() else {
            return Err(format!("vfx.tsv: bad row {line:?}"));
        };
        let num = |v: &str| v.parse::<f32>().map_err(|_| format!("vfx.tsv: {v:?}"));
        index
            .entry(((*recipe).into(), (*team).into(), (*phase).into()))
            .or_default()
            .push(VfxFrame {
                ordinal: ordinal
                    .parse()
                    .map_err(|_| "vfx.tsv: ordinal".to_string())?,
                ms: num(ms)?,
                pivot: Vec2::new(num(px)?, num(py)?),
                size: Vec2::new(num(w)?, num(h)?),
            });
    }
    for frames in index.values_mut() {
        frames.sort_by(|a, b| a.ms.total_cmp(&b.ms));
    }
    Ok(index)
}

fn locomotion(set: &ClipSet, unit: &str, facing: Facing) -> (String, f32) {
    for clip in ["walk", "run"] {
        if let Some(found) = set.loco.get(&(unit.to_string(), facing, clip.to_string())) {
            if found.abs() > 1.0 {
                return (clip.into(), found.abs());
            }
        }
        if set.clip(unit, facing, clip).is_some() {
            // Mounted gait has no planted contacts to derive speed from.
            return (clip.into(), 53.6);
        }
    }
    ("idle".into(), 0.0)
}

fn skill_clips(set: &ClipSet, unit: &str, facing: Facing) -> Vec<String> {
    set.clips
        .keys()
        .filter(|(u, f, id)| u == unit && *f == facing && id.starts_with("skill_"))
        .map(|(_, _, id)| id.clone())
        .collect()
}

/// D01 population cap per side (RULES R0); a crowd beyond it is a stress study.
pub const POPULATION_CAP: usize = 24;

/// Builds the storyboard actors for a scenario.
///
/// `half_width_lu` is each unit's D01 footprint half-width projected with the
/// D02/D03 focus camera (10,000 q across the 1,920-unit stage). Within the
/// population cap, a crowd is one file of touching, non-overlapping footprints
/// (RULES: one lane, no overtaking); beyond it, extra depth rows are a
/// labelled visual stress only.
pub fn actors(
    set: &ClipSet,
    kind: Kind,
    crowd_per_side: usize,
    half_width_lu: &BTreeMap<String, f32>,
) -> Vec<Actor> {
    let units: Vec<&str> = set.units.iter().map(|u| u.id.as_str()).collect();
    let mut out = Vec::new();
    for team in 0..2 {
        let mut cursor = 0.0_f32;
        let facing = if team == 0 {
            Facing::Right
        } else {
            Facing::Left
        };
        let sign = if team == 0 { -1.0 } else { 1.0 };
        let count = match kind {
            Kind::Lineup => units.len(),
            Kind::Crowd => crowd_per_side,
        };
        for index in 0..count {
            let unit = units[index % units.len()];
            let (loco_clip, loco_speed) = locomotion(set, unit, facing);
            let skills = skill_clips(set, unit, facing);
            let (steps, home_x, row, offset_ms) = match kind {
                Kind::Lineup => {
                    let mut steps = vec![
                        Step::Idle(1200.0),
                        Step::Move {
                            clip: loco_clip,
                            ms: 2000.0,
                        },
                    ];
                    steps.push(Step::Clip {
                        clip: "attack".into(),
                        hold_ms: 150.0,
                    });
                    for skill in skills {
                        steps.push(Step::Clip {
                            clip: skill,
                            hold_ms: 150.0,
                        });
                    }
                    steps.push(Step::Clip {
                        clip: "hit".into(),
                        hold_ms: 120.0,
                    });
                    steps.push(Step::Clip {
                        clip: "death".into(),
                        hold_ms: 900.0,
                    });
                    steps.push(Step::Fade(300.0));
                    (
                        steps,
                        960.0 + sign * (230.0 + 112.0 * index as f32),
                        0.0,
                        index as f32 * 140.0,
                    )
                }
                Kind::Crowd => {
                    let mut steps = vec![Step::Clip {
                        clip: "attack".into(),
                        hold_ms: 0.0,
                    }];
                    steps.extend(
                        skills
                            .into_iter()
                            .take(1)
                            .map(|clip| Step::Clip { clip, hold_ms: 0.0 }),
                    );
                    steps.push(Step::Clip {
                        clip: "attack".into(),
                        hold_ms: 0.0,
                    });
                    steps.push(Step::Clip {
                        clip: "hit".into(),
                        hold_ms: 0.0,
                    });
                    steps.push(Step::Move {
                        clip: loco_clip,
                        ms: 600.0,
                    });
                    let half = half_width_lu.get(unit).copied().unwrap_or(40.0);
                    let (centre, row) = if crowd_per_side <= POPULATION_CAP {
                        let centre = cursor + half;
                        cursor += 2.0 * half;
                        (centre, 0.0)
                    } else {
                        let rank = (index / 3) as f32;
                        (half + rank * half, [0.0, -9.0, 9.0][index % 3])
                    };
                    (
                        steps,
                        960.0 + sign * centre,
                        row,
                        (index * 97 % 1300) as f32,
                    )
                }
            };
            let cycle_ms = steps.iter().map(|s| s.duration(set, unit, facing)).sum();
            out.push(Actor {
                unit: unit.into(),
                facing,
                team,
                home_x,
                row,
                offset_ms,
                steps,
                cycle_ms,
                loco_speed: if kind == Kind::Crowd {
                    loco_speed * 0.15
                } else {
                    loco_speed
                },
            });
        }
    }
    out
}

impl Actor {
    fn sign(&self) -> f32 {
        if self.facing == Facing::Right {
            1.0
        } else {
            -1.0
        }
    }

    /// Step index, time within that step and metres walked before it.
    fn locate(&self, set: &ClipSet, time_ms: f32) -> (usize, f32, f32) {
        let local = (time_ms - self.offset_ms).rem_euclid(self.cycle_ms.max(1.0));
        let mut start = 0.0;
        let mut walked = 0.0;
        for (index, step) in self.steps.iter().enumerate() {
            let duration = step.duration(set, &self.unit, self.facing);
            if local < start + duration || index + 1 == self.steps.len() {
                return (index, local - start, walked);
            }
            if let Step::Move { ms, .. } = step {
                walked += self.loco_speed * ms / 1000.0;
            }
            start += duration;
        }
        (0, 0.0, 0.0)
    }

    pub fn pose<'a>(&self, set: &'a ClipSet, time_ms: f32) -> Option<Pose<'a>> {
        let (index, within, walked) = self.locate(set, time_ms);
        let mut x = self.home_x + self.sign() * walked;
        let (clip_id, opacity, clip_time) = match &self.steps[index] {
            Step::Idle(_) => ("idle", 1.0, within),
            Step::Move { clip, .. } => {
                x += self.sign() * self.loco_speed * within / 1000.0;
                (clip.as_str(), 1.0, within)
            }
            Step::Clip { clip, .. } => (clip.as_str(), 1.0, within),
            Step::Fade(ms) => ("death", 1.0 - within / ms, f32::MAX),
        };
        let clip = set.clip(&self.unit, self.facing, clip_id)?;
        Some(Pose {
            clip,
            frame: clip.frame_index(clip_time),
            x,
            opacity,
        })
    }

    /// Cues for markers crossed in `(from, to]` and for hit-step starts.
    pub fn cues(&self, set: &ClipSet, from_ms: f32, to_ms: f32, out: &mut Vec<Cue>) {
        if to_ms <= from_ms {
            return;
        }
        let unit = set.unit(&self.unit);
        let (weapon, impact) =
            unit.map_or(("", ""), |u| (u.vfx_weapon.as_str(), u.vfx_impact.as_str()));
        let (from_index, from_within, _) = self.locate(set, from_ms);
        let (to_index, to_within, _) = self.locate(set, to_ms);
        // Each span carries its step's global start, so a cue is born at the
        // marker's exact presentation time, independent of the frame grid.
        let to_start = to_ms - to_within;
        let mut spans = vec![(
            to_index,
            if from_index == to_index {
                from_within
            } else {
                -1.0
            },
            to_within,
            to_start,
        )];
        if from_index != to_index {
            let end = self.steps[from_index].duration(set, &self.unit, self.facing);
            spans.push((from_index, from_within, end, from_ms - from_within));
        }
        for (index, from, to, start) in spans {
            let Step::Clip { clip, .. } = &self.steps[index] else {
                continue;
            };
            let Some(clip) = set.clip(&self.unit, self.facing, clip) else {
                continue;
            };
            if clip.id == "hit" && from < 0.0 {
                out.push(self.cue(impact, "contact", start, "hit", Vec2::new(0.0, -48.0)));
            }
            for (at, marker) in clip.markers_between(from, to.min(clip.duration_ms)) {
                let frame = &clip.frames[clip.frame_index(at)];
                let socket = frame.muzzle.or(frame.effect).unwrap_or((24.0, -48.0));
                let recipe = match (marker.kind.as_str(), clip.skill.as_deref()) {
                    ("contact" | "release", _) => weapon,
                    (_, Some(skill)) => skill,
                    _ => weapon,
                };
                let kind = if clip.skill.is_some() {
                    "skill"
                } else {
                    "attack"
                };
                out.push(self.cue(
                    recipe,
                    "contact",
                    start + at,
                    kind,
                    Vec2::new(socket.0, socket.1),
                ));
            }
        }
    }

    fn cue(
        &self,
        recipe: &str,
        phase: &'static str,
        now: f32,
        kind: &'static str,
        offset: Vec2,
    ) -> Cue {
        Cue {
            recipe: recipe.into(),
            team: self.team,
            phase,
            born_ms: now,
            at: offset,
            kind,
            unit: self.unit.clone(),
            ground_x: None,
        }
    }
}

/// Logical layout of the stage inside a viewport.
#[derive(Clone, Copy, Debug)]
pub struct Stage {
    pub scale: f32,
    pub top: f32,
}

impl Stage {
    pub fn fit(viewport: Vec2) -> Self {
        let scale = viewport.x / STAGE.x;
        let top = ((viewport.y - STAGE.y * scale) * 0.3).max(0.0);
        Self { scale, top }
    }

    pub fn ground(self, x: f32, row: f32) -> Vec2 {
        Vec2::new(x * self.scale, self.top + (GROUND_Y + row) * self.scale)
    }
}

fn rect(origin: Vec2, size: Vec2) -> Result<Rect, RenderListError> {
    Rect::new(origin, size.max(Vec2::splat(0.01)))
}

fn sprite(asset: &str, area: Rect, tint: Color, layer: Layer, z: i16) -> Result<RenderCmd, String> {
    Ok(RenderCmd::Sprite {
        asset: AssetRef::new(asset).map_err(|e| format!("{asset}: {e}"))?,
        rect: area,
        tint,
        rotation: 0.0,
        pivot: area.origin(),
        layer,
        z,
    })
}

fn with_alpha(color: Color, alpha: f32) -> Color {
    color.with_alpha((f32::from(color.alpha()) * alpha.clamp(0.0, 1.0)) as u8)
}

/// Draws the scene: backdrop, ground markers, actors with team masks, cues, label.
#[allow(clippy::too_many_arguments)]
pub fn draw(
    builder: &mut RenderListBuilder,
    theme: &Theme,
    set: &ClipSet,
    stage: Stage,
    actors: &[Actor],
    time_ms: f32,
    cues: &[Cue],
    vfx: &VfxIndex,
    effects: Effects,
    label: &str,
) -> Result<usize, String> {
    let map = |e: RenderListError| format!("{e:?}");
    // tokens.toml documents this existing art role as the multiplicative
    // identity for coloured raster art. The harness borrows only that neutral
    // tint; a shared production raster role belongs to the C03 token review.
    let white = theme.game_art.chess.piece_tint; // xtask-allow-game-id: existing untinted raster token, no dispatch
    builder
        .push(sprite(
            "scene/backdrop",
            rect(Vec2::new(0.0, stage.top), STAGE * stage.scale).map_err(map)?,
            white,
            Layer::BOARD,
            0,
        )?)
        .map_err(map)?;
    let mut ordered: Vec<(&Actor, Pose<'_>)> = actors
        .iter()
        .filter_map(|a| a.pose(set, time_ms).map(|p| (a, p)))
        .collect();
    ordered.sort_by(|a, b| a.0.row.total_cmp(&b.0.row));
    let mut sprites = 0;
    for (z, (actor, pose)) in ordered.iter().enumerate() {
        let team = theme.color.team[actor.team];
        let feet = stage.ground(pose.x, actor.row);
        let s = stage.scale;
        // Shape-coded ground marker: circle for team A, diamond for team B.
        let marker: Vec<Vec2> = if actor.team == 0 {
            (0..8)
                .map(|i| {
                    let a = i as f32 * std::f32::consts::TAU / 8.0;
                    feet + Vec2::new(a.cos() * 13.0 * s, a.sin() * 4.0 * s)
                })
                .collect()
        } else {
            vec![
                feet + Vec2::new(-14.0 * s, 0.0),
                feet + Vec2::new(0.0, -5.0 * s),
                feet + Vec2::new(14.0 * s, 0.0),
                feet + Vec2::new(0.0, 5.0 * s),
            ]
        };
        builder
            .push(RenderCmd::Path {
                points: marker.into_iter().collect(),
                stroke: Border::new((1.5 * s).max(1.0), with_alpha(team, pose.opacity))
                    .map_err(map)?,
                closed: true,
                fill: Some(Paint::Solid(with_alpha(team, 0.35 * pose.opacity))),
                layer: Layer::BOARD,
                z: 1,
            })
            .map_err(map)?;
        let frame = &pose.clip.frames[pose.frame];
        let id = format!(
            "{}/{}/{}/{:03}",
            actor.unit,
            actor.facing.as_str(),
            pose.clip.id,
            pose.frame
        );
        let body = rect(
            feet + Vec2::new(frame.body.x, frame.body.y) * s,
            Vec2::new(frame.body.w, frame.body.h) * s,
        )
        .map_err(map)?;
        let z = i16::try_from(z).unwrap_or(i16::MAX);
        builder
            .push(sprite(
                &format!("unit/{id}"),
                body,
                with_alpha(white, pose.opacity),
                Layer::PIECES,
                z,
            )?)
            .map_err(map)?;
        sprites += 1;
        if let Some(mask) = frame.mask {
            let area = rect(
                feet + Vec2::new(mask.x, mask.y) * s,
                Vec2::new(mask.w, mask.h) * s,
            )
            .map_err(map)?;
            builder
                .push(sprite(
                    &format!("mask/{id}"),
                    area,
                    with_alpha(team, 0.9 * pose.opacity),
                    Layer::PIECES,
                    z,
                )?)
                .map_err(map)?;
            sprites += 1;
        }
    }
    for cue in cues {
        let team = if cue.team == 0 { "A" } else { "B" };
        let Some(frames) = vfx.get(&(cue.recipe.clone(), team.into(), cue.phase.into())) else {
            continue;
        };
        let age = time_ms - cue.born_ms;
        let first = frames[0].ms;
        let (frame, alpha) = match effects {
            Effects::Full => (
                frames
                    .iter()
                    .rev()
                    .find(|f| f.ms - first <= age)
                    .unwrap_or(&frames[0]),
                1.0,
            ),
            Effects::Low => (
                if age < 90.0 {
                    &frames[0]
                } else {
                    frames.last().unwrap_or(&frames[0])
                },
                0.85,
            ),
            Effects::Reduced => (&frames[0], 0.6),
        };
        let feet = if let Some(x) = cue.ground_x {
            stage.ground(x, 0.0)
        } else {
            let actor = actors
                .iter()
                .find(|a| a.unit == cue.unit && a.team == cue.team);
            let Some(pose) = actor.and_then(|a| a.pose(set, cue.born_ms).map(|p| (a, p))) else {
                continue;
            };
            stage.ground(pose.1.x, pose.0.row)
        };
        let size = frame.size * VFX_LU_PER_TEXEL * stage.scale * 0.5;
        let origin =
            feet + cue.at * stage.scale - frame.pivot * VFX_LU_PER_TEXEL * stage.scale * 0.5;
        let asset = format!("vfx/{}/{team}/{}/{}", cue.recipe, cue.phase, frame.ordinal);
        builder
            .push(sprite(
                &asset,
                rect(origin, size).map_err(map)?,
                with_alpha(white, alpha),
                Layer::OVERLAY,
                0,
            )?)
            .map_err(map)?;
        sprites += 1;
    }
    builder
        .push(RenderCmd::Text {
            text: label.into(),
            at: Vec2::new(12.0, 10.0),
            style: TextStyleToken::LabelMd,
            align: Align::Start,
            max_width: None,
            color: theme.color.on_surface,
            layer: Layer::HUD,
            z: 0,
        })
        .map_err(map)?;
    Ok(sprites)
}

/// Cue lifetime (ms) for pruning, by effects mode.
pub fn cue_lifetime(vfx: &VfxIndex, cue: &Cue, effects: Effects) -> f32 {
    let team = if cue.team == 0 { "A" } else { "B" };
    let span = vfx
        .get(&(cue.recipe.clone(), team.into(), cue.phase.into()))
        .map_or(0.0, |f| f.last().map_or(0.0, |l| l.ms) - f[0].ms);
    match effects {
        Effects::Full => span + 120.0,
        Effects::Low => 180.0,
        Effects::Reduced => 300.0,
    }
}
