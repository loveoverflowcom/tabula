//! Integrity-verified clip sidecar (`meta/clips.tsv`) and elapsed-time sampling.
//!
//! Presentation facts only: a marker is a non-authoritative cue and never a hit
//! (doc 00 I-10). Every geometric value is in stage logical units relative to
//! the frame's ground pivot (feet), already scaled by the unit's display factor.

use std::collections::BTreeMap;

/// Which way a unit faces. Each side of a match only ever shows one facing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Facing {
    Right,
    Left,
}

impl Facing {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Right => "right",
            Self::Left => "left",
        }
    }

    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "right" => Ok(Self::Right),
            "left" => Ok(Self::Left),
            other => Err(format!("unknown facing {other:?}")),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct UnitInfo {
    pub id: String,
    pub age: String,
    pub role: String,
    pub hint: String,
    pub body_lu: f32,
    pub lu_per_bake_px: f32,
    pub weapon: String,
    pub skills: Vec<String>,
    pub vfx_weapon: String,
    pub vfx_impact: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Marker {
    pub time_ms: f32,
    pub kind: String,
    pub cue: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Box2 {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    pub time_ms: f32,
    pub body: Box2,
    pub mask: Option<Box2>,
    pub effect: Option<(f32, f32)>,
    pub muzzle: Option<(f32, f32)>,
    pub grip: Option<(f32, f32)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Clip {
    pub unit: String,
    pub facing: Facing,
    pub id: String,
    pub skill: Option<String>,
    pub duration_ms: f32,
    pub looping: bool,
    pub windup_ms: Option<f32>,
    pub markers: Vec<Marker>,
    pub frames: Vec<Frame>,
}

/// Parsed sidecar for one age pack.
#[derive(Clone, Debug, Default)]
pub struct ClipSet {
    pub pack: String,
    pub age: String,
    pub bake_hz: u32,
    pub units: Vec<UnitInfo>,
    pub clips: BTreeMap<(String, Facing, String), Clip>,
    /// Planted-foot ground speed (lu/s, + = screen right) per locomotion clip.
    pub loco: BTreeMap<(String, Facing, String), f32>,
}

fn number(value: &str, field: &str) -> Result<f32, String> {
    let parsed: f32 = value
        .parse()
        .map_err(|_| format!("{field}: not a number {value:?}"))?;
    if parsed.is_finite() {
        Ok(parsed)
    } else {
        Err(format!("{field}: non-finite"))
    }
}

fn optional(value: &str, field: &str) -> Result<Option<f32>, String> {
    if value == "-" {
        Ok(None)
    } else {
        number(value, field).map(Some)
    }
}

fn point(x: &str, y: &str, field: &str) -> Result<Option<(f32, f32)>, String> {
    Ok(match (optional(x, field)?, optional(y, field)?) {
        (Some(x), Some(y)) => Some((x, y)),
        (None, None) => None,
        _ => return Err(format!("{field}: half-specified point")),
    })
}

impl ClipSet {
    /// Parses and validates the whole sidecar; any malformed row rejects the set.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut set = Self::default();
        let mut expected_frames = BTreeMap::new();
        for (line_number, line) in text.lines().enumerate() {
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            let at = |message: String| format!("clips.tsv:{}: {message}", line_number + 1);
            let fields: Vec<&str> = line.split('\t').collect();
            match fields.as_slice() {
                ["pack", pack, age, hz] => {
                    set.pack = (*pack).to_string();
                    set.age = (*age).to_string();
                    set.bake_hz = hz.parse().map_err(|_| at("bad bake rate".into()))?;
                }
                ["unit", id, age, role, hint, body, k, _t, weapon, skills, vfx_weapon, vfx_impact] =>
                {
                    set.units.push(UnitInfo {
                        id: (*id).to_string(),
                        age: (*age).to_string(),
                        role: (*role).to_string(),
                        hint: (*hint).to_string(),
                        body_lu: number(body, "body").map_err(at)?,
                        lu_per_bake_px: number(k, "k").map_err(at)?,
                        weapon: (*weapon).to_string(),
                        skills: skills.split(',').map(str::to_string).collect(),
                        vfx_weapon: (*vfx_weapon).to_string(),
                        vfx_impact: (*vfx_impact).to_string(),
                    });
                }
                ["loco", unit, facing, id, speed, _slide] => {
                    let key = (
                        (*unit).to_string(),
                        Facing::parse(facing).map_err(at)?,
                        (*id).to_string(),
                    );
                    if !set.clips.contains_key(&key) {
                        return Err(at("locomotion before its clip".into()));
                    }
                    set.loco.insert(key, number(speed, "speed").map_err(at)?);
                }
                ["clip", unit, facing, id, skill, duration, looping, windup, frames] => {
                    let facing = Facing::parse(facing).map_err(at)?;
                    let duration_ms = number(duration, "duration").map_err(at)?;
                    if duration_ms <= 0.0 {
                        return Err(at("non-positive duration".into()));
                    }
                    let key = ((*unit).to_string(), facing, (*id).to_string());
                    expected_frames.insert(
                        key.clone(),
                        frames
                            .parse::<usize>()
                            .map_err(|_| at("bad frame count".into()))?,
                    );
                    let clip = Clip {
                        unit: (*unit).to_string(),
                        facing,
                        id: (*id).to_string(),
                        skill: (*skill != "-").then(|| (*skill).to_string()),
                        duration_ms,
                        looping: *looping == "1",
                        windup_ms: optional(windup, "windup").map_err(at)?,
                        markers: Vec::new(),
                        frames: Vec::new(),
                    };
                    if set.clips.insert(key, clip).is_some() {
                        return Err(at("duplicate clip".into()));
                    }
                }
                ["marker", unit, facing, id, time, kind, cue] => {
                    let key = (
                        (*unit).to_string(),
                        Facing::parse(facing).map_err(at)?,
                        (*id).to_string(),
                    );
                    let clip = set
                        .clips
                        .get_mut(&key)
                        .ok_or_else(|| at("marker before its clip".into()))?;
                    let time_ms = number(time, "marker").map_err(at)?;
                    if !(0.0..=clip.duration_ms).contains(&time_ms) {
                        return Err(at("marker outside clip".into()));
                    }
                    clip.markers.push(Marker {
                        time_ms,
                        kind: (*kind).to_string(),
                        cue: (*cue != "-").then(|| (*cue).to_string()),
                    });
                }
                ["frame", unit, facing, id, index, time, x, y, w, h, mx, my, mw, mh, ex, ey, zx, zy, gx, gy] =>
                {
                    let key = (
                        (*unit).to_string(),
                        Facing::parse(facing).map_err(at)?,
                        (*id).to_string(),
                    );
                    let clip = set
                        .clips
                        .get_mut(&key)
                        .ok_or_else(|| at("frame before its clip".into()))?;
                    if index.parse::<usize>().ok() != Some(clip.frames.len()) {
                        return Err(at("frames out of order".into()));
                    }
                    let time_ms = number(time, "time").map_err(at)?;
                    if clip
                        .frames
                        .last()
                        .is_some_and(|last| last.time_ms >= time_ms)
                        || time_ms > clip.duration_ms
                    {
                        return Err(at("frame time not increasing or beyond clip".into()));
                    }
                    let body = Box2 {
                        x: number(x, "x").map_err(at)?,
                        y: number(y, "y").map_err(at)?,
                        w: number(w, "w").map_err(at)?,
                        h: number(h, "h").map_err(at)?,
                    };
                    if body.w <= 0.0 || body.h <= 0.0 {
                        return Err(at("empty frame".into()));
                    }
                    let mask = match (
                        optional(mx, "mx"),
                        optional(my, "my"),
                        optional(mw, "mw"),
                        optional(mh, "mh"),
                    ) {
                        (Ok(Some(x)), Ok(Some(y)), Ok(Some(w)), Ok(Some(h))) => {
                            Some(Box2 { x, y, w, h })
                        }
                        (Ok(None), Ok(None), Ok(None), Ok(None)) => None,
                        _ => return Err(at("malformed mask box".into())),
                    };
                    clip.frames.push(Frame {
                        time_ms,
                        body,
                        mask,
                        effect: point(ex, ey, "effect").map_err(at)?,
                        muzzle: point(zx, zy, "muzzle").map_err(at)?,
                        grip: point(gx, gy, "grip").map_err(at)?,
                    });
                }
                _ => return Err(at(format!("unrecognized row with {} fields", fields.len()))),
            }
        }
        for (key, count) in expected_frames {
            let clip = &set.clips[&key];
            if clip.frames.len() != count || clip.frames.first().map(|f| f.time_ms) != Some(0.0) {
                return Err(format!(
                    "clip {key:?}: declared {count} frames, found {}",
                    clip.frames.len()
                ));
            }
        }
        if set.pack.is_empty() || set.units.is_empty() {
            return Err("clips.tsv lacks pack or unit rows".into());
        }
        Ok(set)
    }

    pub fn clip(&self, unit: &str, facing: Facing, id: &str) -> Option<&Clip> {
        self.clips.get(&(unit.to_string(), facing, id.to_string()))
    }

    pub fn unit(&self, id: &str) -> Option<&UnitInfo> {
        self.units.iter().find(|unit| unit.id == id)
    }
}

impl Clip {
    /// Elapsed presentation time on this clip's own timeline: a loop wraps and a
    /// one-shot holds its final pose. Speed is applied by the caller.
    pub fn local_time(&self, elapsed_ms: f32) -> f32 {
        let elapsed = elapsed_ms.max(0.0);
        if self.looping {
            elapsed.rem_euclid(self.duration_ms)
        } else {
            elapsed.min(self.duration_ms)
        }
    }

    /// The baked pose shown at an elapsed time: the last frame not after it.
    pub fn frame_index(&self, elapsed_ms: f32) -> usize {
        let local = self.local_time(elapsed_ms);
        self.frames
            .partition_point(|frame| frame.time_ms <= local + 1e-3)
            .saturating_sub(1)
    }

    /// Markers whose elapsed occurrence lies in `(from, to]`, plus a zero-time
    /// marker when the clip starts (`from < 0`). Loop cycles are enumerated so
    /// sparse and dense sampling emit the same ordered cues.
    pub fn markers_between(&self, from_ms: f32, to_ms: f32) -> Vec<(f32, &Marker)> {
        let mut out = Vec::new();
        if to_ms <= from_ms {
            return out;
        }
        let cycles = if self.looping {
            let first = (from_ms.max(0.0) / self.duration_ms).floor() as i64;
            let last = (to_ms / self.duration_ms).floor() as i64;
            first..=last.min(first + 4096)
        } else {
            0..=0
        };
        for cycle in cycles {
            for marker in &self.markers {
                let at = marker.time_ms + cycle as f32 * self.duration_ms;
                let started = from_ms < 0.0 && at == 0.0;
                if (at > from_ms && at <= to_ms) || started {
                    out.push((at, marker));
                }
            }
        }
        out.sort_by(|a, b| a.0.total_cmp(&b.0));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "# test\npack\tage-war-test\tPrimitive\t24\n\
unit\ta\tPrimitive\tFrontline\tMasculineHuman\t96\t0.5\t0.5\tclub\tguard\tweapon_club\timpact_physical\n\
clip\ta\tright\tidle\t-\t100\t1\t-\t2\n\
marker\ta\tright\tidle\t0\tcue\t-\n\
marker\ta\tright\tidle\t50\tcue\t-\n\
frame\ta\tright\tidle\t0\t0\t-10\t-90\t20\t90\t-\t-\t-\t-\t-\t-\t-\t-\t-\t-\n\
frame\ta\tright\tidle\t1\t50\t-10\t-90\t20\t90\t-2\t-40\t4\t5\t3\t-50\t3\t-50\t1\t-45\n\
clip\ta\tright\tattack\t-\t300\t0\t100\t2\n\
marker\ta\tright\tattack\t100\tcontact\t-\n\
frame\ta\tright\tattack\t0\t0\t-10\t-90\t20\t90\t-\t-\t-\t-\t-\t-\t-\t-\t-\t-\n\
frame\ta\tright\tattack\t1\t300\t-10\t-90\t20\t90\t-\t-\t-\t-\t-\t-\t-\t-\t-\t-\n";

    #[test]
    fn sidecar_parses_and_rejects_malformed_rows() {
        let set = ClipSet::parse(SAMPLE).unwrap();
        assert_eq!(set.clips.len(), 2);
        assert_eq!(
            set.clip("a", Facing::Right, "idle").unwrap().frames[1]
                .mask
                .unwrap()
                .w,
            4.0
        );
        for broken in [
            SAMPLE.replace("\t100\t1\t-\t2", "\t100\t1\t-\t3"),
            SAMPLE.replace(
                "frame\ta\tright\tidle\t1\t50",
                "frame\ta\tright\tidle\t1\t0",
            ),
            SAMPLE.replace(
                "marker\ta\tright\tattack\t100",
                "marker\ta\tright\tattack\t301",
            ),
            SAMPLE.replace("\t-2\t-40\t4\t5", "\t-2\t-\t4\t5"),
            SAMPLE.replace("\t-10\t-90\t20\t90", "\t-10\t-90\tNaN\t90"),
            format!("{SAMPLE}bogus\trow\n"),
        ] {
            assert!(ClipSet::parse(&broken).is_err(), "{broken}");
        }
    }

    #[test]
    fn sampling_wraps_loops_and_holds_one_shots() {
        let set = ClipSet::parse(SAMPLE).unwrap();
        let idle = set.clip("a", Facing::Right, "idle").unwrap();
        assert_eq!(idle.frame_index(49.0), 0);
        assert_eq!(idle.frame_index(50.0), 1);
        assert_eq!(idle.frame_index(149.0), 0);
        let attack = set.clip("a", Facing::Right, "attack").unwrap();
        assert_eq!(attack.frame_index(10_000.0), 1);
    }

    #[test]
    fn sparse_and_dense_marker_dispatch_agree_at_every_speed() {
        let set = ClipSet::parse(SAMPLE).unwrap();
        let idle = set.clip("a", Facing::Right, "idle").unwrap();
        for speed in [0.5_f32, 1.0, 2.0] {
            // One wall millisecond advances clip time by `speed`.
            let horizon = 351.0;
            let sparse: Vec<f32> = idle
                .markers_between(-1.0, horizon)
                .iter()
                .map(|m| m.0)
                .collect();
            let mut dense = Vec::new();
            let mut previous = -1.0;
            let mut now = 0.0;
            while now <= horizon {
                dense.extend(idle.markers_between(previous, now).iter().map(|m| m.0));
                previous = now;
                now += 1.0 * speed;
            }
            assert_eq!(sparse, dense, "speed {speed}");
            assert_eq!(sparse.len(), 8);
        }
    }
}
