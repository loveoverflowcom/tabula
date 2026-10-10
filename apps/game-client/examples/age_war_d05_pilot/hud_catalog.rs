//! Public HUD facts exported from the compiled D01 catalog for the D06 design
//! preview. // xtask-allow-game-id: Age War harness export
//!
//! Only already-public design data leaves here; no pack, asset or private
//! delivery fact is read. Ticks are converted at the D01 50 ms quantum.

use std::fmt::Write as _;

use tabula_game_age_war::{
    // xtask-allow-game-id: harness-only catalog export
    catalog::{AGE_ADVANCES, SPELLS, TECHS, TURRETS, UNITS},
    schema::{Age, SkillCost},
};

use crate::json_str;

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

fn seconds(ticks: u16) -> String {
    format!("{}.{:02}", ticks / 20, ticks % 20 * 5)
}

pub fn export() -> String {
    let mut ages = Vec::new();
    for age in Age::ALL {
        let mut units = Vec::new();
        for unit in UNITS.iter().filter(|u| u.age == age) {
            let skills: Vec<String> = unit
                .skills
                .iter()
                .map(|s| json_str(s.id.as_str()))
                .collect();
            units.push(format!(
                "{{\"id\":{},\"name_vi\":{},\"role\":{},\"hint\":{},\"cost\":{},\"pop\":{},\"train_s\":{},\"hp\":{},\"damage_kind\":{},\"reach_q\":{},\"skills\":[{}]}}",
                json_str(unit.id.as_str()), json_str(unit.name_vi), json_str(&format!("{:?}", unit.role)),
                json_str(&format!("{:?}", unit.presentation_hint)), unit.cost_gold, unit.population,
                seconds(unit.train.get()), unit.hp, json_str(&format!("{:?}", unit.attack.kind)),
                unit.attack.reach_q.get(), skills.join(",")
            ));
        }
        let mut spells = Vec::new();
        for spell in SPELLS.iter().filter(|s| s.age == age) {
            let cost = match spell.card.cost {
                SkillCost::Free => 0,
                SkillCost::Gold(gold) => gold,
            };
            spells.push(format!(
                "{{\"id\":{},\"name_vi\":{},\"slot\":{},\"cost\":{cost},\"cooldown_s\":{},\"windup_s\":{},\"aoe_radius_q\":{}}}",
                json_str(spell.card.id.as_str()), json_str(spell.name_vi), json_str(&format!("{:?}", spell.slot)),
                seconds(spell.card.cooldown.get()), seconds(spell.card.windup.get()), spell.card.aoe_radius_q.get()
            ));
        }
        let mut turrets = Vec::new();
        for turret in TURRETS.iter().filter(|t| t.age == age) {
            turrets.push(format!(
                "{{\"id\":{},\"name_vi\":{},\"branch\":{},\"cost\":{},\"build_s\":{}}}",
                json_str(turret.id.as_str()),
                json_str(turret.name_vi),
                json_str(&format!("{:?}", turret.branch)),
                turret.cost_gold,
                seconds(turret.build.get())
            ));
        }
        let mut techs = Vec::new();
        for tech in TECHS.iter().filter(|t| t.age == age) {
            techs.push(format!(
                "{{\"id\":{},\"kind\":{},\"cost\":{},\"research_s\":{},\"bonus_bp\":{}}}",
                json_str(tech.id.as_str()),
                json_str(&format!("{:?}", tech.kind)),
                tech.cost_gold,
                seconds(tech.research.get()),
                tech.bonus_bp.get()
            ));
        }
        let advance =
            AGE_ADVANCES
                .iter()
                .find(|a| a.to_age == age)
                .map_or("null".to_string(), |a| {
                    format!(
                        "{{\"gold\":{},\"total_xp\":{},\"duration_s\":{}}}",
                        a.gold,
                        a.total_xp,
                        seconds(a.duration.get())
                    )
                });
        let mut entry = String::new();
        let _ = write!(
            entry,
            "{{\"age\":{},\"number\":{},\"label_vi\":{},\"advance_to\":{advance},\"units\":[{}],\"spells\":[{}],\"turrets\":[{}],\"techs\":[{}]}}",
            json_str(age_name(age)), age.number(), json_str(age.label_vi()), units.join(","), spells.join(","),
            turrets.join(","), techs.join(",")
        );
        ages.push(entry);
    }
    format!(
        "{{\"source\":\"compiled D01 catalog (design 0.1.0-d01, UNBALANCED)\",\"rules_r0\":{{\"start_gold\":180,\"income_per_s\":5,\"gold_cap\":20000,\"population_cap\":24,\"queue\":5,\"base_hp\":3000,\"turret_sockets\":2,\"spell_slots\":2,\"fatigue_starts_s\":720,\"hard_limit_s\":1200}},\"ages\":[{}]}}",
        ages.join(",")
    )
}
