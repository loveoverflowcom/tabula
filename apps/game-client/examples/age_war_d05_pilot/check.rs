//! Independent oracle: delivered clip timing against the compiled D01 catalog.
//! xtask-allow-game-id: Age War presentation-pilot harness, not platform dispatch.
//!
//! The private sidecar was produced from the D05 delivery; the catalog is the
//! frozen public design authority. Agreement here shows that windup/contact
//! timing was baked from D01, not that the motion looks right.

use std::collections::BTreeMap;

use tabula_game_age_war::{
    // xtask-allow-game-id: harness-only catalog oracle
    catalog::{SKILLS, UNITS},
    schema::{Activation, Age},
};

use crate::clips::{ClipSet, Facing};

/// D01 logical tick (RULES R0).
pub const TICK_MS: f32 = 50.0;

#[derive(Debug, Default)]
pub struct CheckReport {
    pub passed: BTreeMap<&'static str, u32>,
    pub failed: BTreeMap<&'static str, Vec<String>>,
}

impl CheckReport {
    fn record(&mut self, name: &'static str, ok: bool, detail: impl FnOnce() -> String) {
        if ok {
            *self.passed.entry(name).or_default() += 1;
        } else {
            self.failed.entry(name).or_default().push(detail());
        }
    }

    pub fn failures(&self) -> usize {
        self.failed.values().map(Vec::len).sum()
    }
}

fn age_name(age: Age) -> &'static str {
    match age {
        Age::Primitive => "Primitive",
        Age::Ancient => "Ancient",
        Age::Feudal => "Feudal",
        Age::Arcane => "Arcane",
        Age::Industrial => "Industrial",
        Age::Future => "Future",
    }
}

fn close(left: f32, right: f32) -> bool {
    (left - right).abs() <= 0.01
}

/// Checks one age's sidecar against the catalog and appends to `report`.
pub fn check_age(set: &ClipSet, report: &mut CheckReport) {
    let catalog: Vec<_> = UNITS
        .iter()
        .filter(|unit| age_name(unit.age) == set.age)
        .collect();
    let mut delivered: Vec<_> = set.units.iter().map(|unit| unit.id.as_str()).collect();
    let mut expected: Vec<_> = catalog.iter().map(|unit| unit.id.as_str()).collect();
    delivered.sort_unstable();
    expected.sort_unstable();
    report.record("roster_matches_d01", delivered == expected, || {
        format!("{}: delivered {delivered:?} expected {expected:?}", set.age)
    });
    for spec in catalog {
        let id = spec.id.as_str();
        let Some(unit) = set.unit(id) else { continue };
        report.record(
            "role_matches_d01",
            unit.role == format!("{:?}", spec.role),
            || id.into(),
        );
        report.record(
            "presentation_hint_matches_d01",
            unit.hint == format!("{:?}", spec.presentation_hint),
            || id.into(),
        );
        let skills: Vec<_> = spec
            .skills
            .iter()
            .map(|binding| binding.id.as_str())
            .collect();
        report.record("skills_match_d01", unit.skills == skills, || {
            format!("{id}: {:?}", unit.skills)
        });
        let windup = f32::from(spec.attack.windup.get()) * TICK_MS;
        let period =
            (f32::from(spec.attack.windup.get()) + f32::from(spec.attack.recovery.get())) * TICK_MS;
        for facing in [Facing::Right, Facing::Left] {
            let label = |clip: &str| format!("{id}/{}/{clip}", facing.as_str());
            for required in ["idle", "attack", "hit", "death"] {
                report.record(
                    "required_clip",
                    set.clip(id, facing, required).is_some(),
                    || label(required),
                );
            }
            report.record(
                "locomotion_clip",
                set.clip(id, facing, "walk").is_some() || set.clip(id, facing, "run").is_some(),
                || label("walk|run"),
            );
            if let Some(attack) = set.clip(id, facing, "attack") {
                report.record(
                    "attack_windup_eq_d01",
                    attack.windup_ms.is_some_and(|w| close(w, windup)),
                    || format!("{}: {:?} vs {windup}", label("attack"), attack.windup_ms),
                );
                report.record(
                    "attack_period_eq_d01",
                    close(attack.duration_ms, period),
                    || format!("{}: {} vs {period}", label("attack"), attack.duration_ms),
                );
                // RULES R2/R3: melee resolves at contact; a projectile is released
                // at windup and hits later through its own arrival fact.
                let expected = if spec.attack.projectile_speed_q_per_tick.get() == 0 {
                    "contact"
                } else {
                    "release"
                };
                report.record(
                    "attack_marker_kind_and_time_eq_d01",
                    attack
                        .markers
                        .iter()
                        .any(|m| m.kind == expected && close(m.time_ms, windup)),
                    || format!("{} expected {expected}", label("attack")),
                );
            }
            for skill_id in &skills {
                let Some(card) = SKILLS.iter().find(|card| card.id.as_str() == *skill_id) else {
                    report.record("skill_in_d01", false, || label(skill_id));
                    continue;
                };
                let timed = set
                    .clips
                    .values()
                    .filter(|clip| clip.unit == id && clip.facing == facing)
                    .find(|clip| clip.skill.as_deref() == Some(*skill_id));
                match (timed, card.activation) {
                    (Some(clip), _) if clip.id.starts_with("skill_") => {
                        let windup = f32::from(card.windup.get()) * TICK_MS;
                        let period = (f32::from(card.windup.get())
                            + f32::from(card.recovery.get()))
                            * TICK_MS;
                        report.record(
                            "skill_windup_eq_d01",
                            clip.windup_ms.is_some_and(|w| close(w, windup)),
                            || format!("{}: {:?} vs {windup}", label(&clip.id), clip.windup_ms),
                        );
                        report.record(
                            "skill_period_eq_d01",
                            close(clip.duration_ms, period),
                            || format!("{}: {} vs {period}", label(&clip.id), clip.duration_ms),
                        );
                        report.record(
                            "skill_cue_marker_is_d01_contact_key",
                            clip.markers
                                .iter()
                                .any(|m| m.cue.as_deref() == Some(card.cues.contact.as_str())),
                            || label(&clip.id),
                        );
                    }
                    (Some(clip), _) => {
                        report.record("skill_motion_clip_present", !clip.frames.is_empty(), || {
                            label(&clip.id)
                        });
                    }
                    (None, Activation::Passive | Activation::OnAttackContact) => {
                        report.record(
                            "passive_skill_reuses_attack",
                            set.clip(id, facing, "attack").is_some(),
                            || label(skill_id),
                        );
                    }
                    (None, activation) => report.record("timed_skill_has_clip", false, || {
                        format!("{} {activation:?}", label(skill_id))
                    }),
                }
            }
            if let (Some(right), Some(left)) = (
                set.clip(id, Facing::Right, "idle"),
                set.clip(id, Facing::Left, "idle"),
            ) {
                report.record(
                    "facing_frame_parity_idle",
                    right.frames.len() == left.frames.len(),
                    || id.into(),
                );
            }
        }
        let right: Vec<_> = set
            .clips
            .keys()
            .filter(|k| k.0 == id && k.1 == Facing::Right)
            .map(|k| &k.2)
            .collect();
        let left: Vec<_> = set
            .clips
            .keys()
            .filter(|k| k.0 == id && k.1 == Facing::Left)
            .map(|k| &k.2)
            .collect();
        report.record("facing_clip_parity", right == left, || id.into());
    }
}
