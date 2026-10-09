//! Schema/catalog examples only. No simulator, gameplay or SDK conformance claim.

use std::collections::BTreeSet;

use tabula_game_age_war::{
    catalog::{
        AGE_ADVANCES, CONTROL_IMMUNITY, DESIGN_VERSION, DRONES, DRONES_PER_MATCH_CAP,
        DRONES_PER_OWNER_CAP, LIFETIME_HEAL_CAP_BP, LIFETIME_SHIELD_CAP_BP,
        PROJECTILES_PER_MATCH_CAP, PROJECTILE_LIFETIME_CAP, SKILLS, SPELLS,
        STATUSES_PER_ENTITY_CAP, TECHS, TURRETS, UNBALANCED, UNITS,
    },
    phase_gate::{
        request_ai_plan, request_simulation, request_snapshot_migration, DeferredSurface,
        PhaseGateError,
    },
    schema::{
        Activation, Age, BasisPoints, Contact, ExpiryPolicy, Id, Immunity, Mitigation,
        PresentationHint, SchemaError, SkillBinding, SkillCost, SkillEffect, SkillFamily,
        SpellSlot, StackPolicy, TargetFilter, TargetSide, TechKind, Ticks, TurretBranch, UnitRole,
        MAX_Q, MAX_TICKS, Q,
    },
};

#[test]
fn exact_d01_inventory_is_unbalanced_and_not_runtime_versioned() {
    const { assert!(UNBALANCED) };
    assert_eq!(DESIGN_VERSION, 1);
    assert_eq!(UNITS.len(), 36);
    assert_eq!(SKILLS.len(), 16);
    assert_eq!(SPELLS.len(), 12);
    assert_eq!(TURRETS.len(), 12);
    assert_eq!(TECHS.len(), 12);
    assert_eq!(AGE_ADVANCES.len(), 5);
}

#[test]
fn every_authored_descriptor_crosses_its_validation_boundary() {
    for unit in &UNITS {
        assert!(unit.validate().is_ok(), "unit {}", unit.id.as_str());
    }
    for card in &SKILLS {
        assert!(card.validate().is_ok(), "skill {}", card.id.as_str());
    }
    for spell in &SPELLS {
        assert!(
            spell.card.validate().is_ok(),
            "spell {}",
            spell.card.id.as_str()
        );
        assert_eq!(spell.card.activation, Activation::CommanderCast);
        assert!(!spell.name_vi.is_empty());
    }
    for turret in &TURRETS {
        assert!(turret.validate().is_ok(), "turret {}", turret.id.as_str());
    }
    for tech in &TECHS {
        assert!(tech.validate().is_ok(), "tech {}", tech.id.as_str());
    }
}

#[test]
fn authored_literals_use_unique_valid_ids_across_all_content() {
    let mut ids = BTreeSet::new();
    let all = UNITS
        .iter()
        .map(|u| u.id)
        .chain(SKILLS.iter().map(|s| s.id))
        .chain(SPELLS.iter().map(|s| s.card.id))
        .chain(TURRETS.iter().map(|t| t.id))
        .chain(TECHS.iter().map(|t| t.id))
        .chain(DRONES.iter().map(|d| d.id));
    let mut count = 0;
    for id in all {
        assert_eq!(Id::new(id.as_str()), Ok(id));
        assert!(ids.insert(id.as_str()), "duplicate {}", id.as_str());
        count += 1;
    }
    assert_eq!(count, 90);
}

#[test]
fn each_age_has_all_six_roles_two_spells_two_branches_two_techs() {
    for age in Age::ALL {
        let units: Vec<_> = UNITS.iter().filter(|unit| unit.age == age).collect();
        assert_eq!(units.len(), 6);
        assert_eq!(
            units.iter().map(|unit| unit.role).collect::<BTreeSet<_>>(),
            UnitRole::ALL.into_iter().collect()
        );
        assert_eq!(
            SPELLS
                .iter()
                .filter(|s| s.age == age)
                .map(|s| s.slot)
                .collect::<BTreeSet<_>>(),
            [SpellSlot::First, SpellSlot::Second].into_iter().collect()
        );
        assert_eq!(
            TURRETS
                .iter()
                .filter(|s| s.age == age)
                .map(|s| s.branch)
                .collect::<BTreeSet<_>>(),
            [TurretBranch::Direct, TurretBranch::Control]
                .into_iter()
                .collect()
        );
        assert_eq!(
            TECHS
                .iter()
                .filter(|s| s.age == age)
                .map(|s| s.kind)
                .collect::<BTreeSet<_>>(),
            [TechKind::Attack, TechKind::Armor].into_iter().collect()
        );
    }
    assert_eq!(
        Age::ALL.map(Age::label_vi),
        [
            "Hoang sơ",
            "Cổ đại",
            "Phong kiến",
            "Huyền thuật",
            "Công nghiệp",
            "Tương lai"
        ]
    );
}

fn resolve(binding: SkillBinding) -> SkillFamily {
    let cards: Vec<_> = SKILLS
        .iter()
        .filter(|s| s.id == binding.id && s.design_version == binding.design_version)
        .collect();
    assert_eq!(
        cards.len(),
        1,
        "unresolved/ambiguous {}",
        binding.id.as_str()
    );
    cards[0].family
}

#[test]
fn all_bindings_resolve_exact_versions_and_all_families_are_consumed() {
    let mut consumed = BTreeSet::new();
    for unit in &UNITS {
        for binding in unit.skills {
            consumed.insert(resolve(*binding));
        }
    }
    for turret in &TURRETS {
        for binding in turret.skills {
            let family = resolve(*binding);
            assert!(!matches!(
                family,
                SkillFamily::Heal | SkillFamily::Shield | SkillFamily::Summon
            ));
            consumed.insert(family);
        }
    }
    for spell in &SPELLS {
        consumed.insert(spell.card.family);
    }
    assert_eq!(consumed, SkillFamily::ALL.into_iter().collect());
    assert_eq!(
        SKILLS.iter().map(|s| s.family).collect::<BTreeSet<_>>(),
        consumed
    );
    assert_eq!(SkillFamily::BurstHeat.key(), "burst_heat");
}

#[test]
fn ages_change_tactical_loadouts_instead_of_only_rescaling_six_units() {
    for ages in Age::ALL.windows(2) {
        let mut changed_roles = 0;
        for role in UnitRole::ALL {
            let previous = UNITS
                .iter()
                .find(|u| u.age == ages[0] && u.role == role)
                .unwrap();
            let current = UNITS
                .iter()
                .find(|u| u.age == ages[1] && u.role == role)
                .unwrap();
            let previous_families: BTreeSet<_> =
                previous.skills.iter().map(|s| resolve(*s)).collect();
            let current_families: BTreeSet<_> =
                current.skills.iter().map(|s| resolve(*s)).collect();
            if previous_families != current_families || previous.attack.kind != current.attack.kind
            {
                changed_roles += 1;
            }
        }
        assert!(
            changed_roles >= 2,
            "two tactical changes required: {ages:?}"
        );
    }
}

#[test]
fn art_hint_ratio_is_d02_only_and_has_no_stat_selector() {
    let humans = UNITS
        .iter()
        .filter(|u| u.presentation_hint != PresentationHint::Construct)
        .count();
    let feminine = UNITS
        .iter()
        .filter(|u| u.presentation_hint == PresentationHint::FeminineHuman)
        .count();
    assert_eq!((humans, feminine), (28, 19));
    assert!(feminine * 100 >= humans * 60 && feminine * 100 <= humans * 70);
    // No target filter, payload or role has a gender variant. This count is an
    // authored-art inventory check, never a gameplay or D02 acceptance test.
}

#[test]
fn lifetime_and_live_inventory_caps_are_explicit_nonzero_d01_data() {
    assert_eq!(LIFETIME_HEAL_CAP_BP.get(), 3_500);
    assert_eq!(LIFETIME_SHIELD_CAP_BP.get(), 5_000);
    assert_eq!((DRONES_PER_OWNER_CAP, DRONES_PER_MATCH_CAP), (4, 8));
    assert_eq!(CONTROL_IMMUNITY.get(), 40);
    assert_eq!(PROJECTILE_LIFETIME_CAP.get(), 120);
    assert_eq!(PROJECTILES_PER_MATCH_CAP, 128);
    assert_eq!(STATUSES_PER_ENTITY_CAP, 16);
    for card in SKILLS.iter().chain(SPELLS.iter().map(|s| &s.card)) {
        match card.effect {
            SkillEffect::Root => assert!(card.duration.get() <= 12),
            SkillEffect::Slow { reduction_bp } => assert!(reduction_bp.get() <= 4_000),
            SkillEffect::Summon {
                lifetime, count, ..
            } => {
                assert!(lifetime.get() <= 240);
                assert!(count <= 2);
            }
            _ => {}
        }
    }
}

#[test]
fn advancement_and_research_match_independent_design_expectations() {
    assert_eq!(AGE_ADVANCES.map(|s| s.gold), [250, 400, 600, 850, 1_150]);
    assert_eq!(AGE_ADVANCES.map(|s| s.total_xp), [60, 140, 240, 360, 500]);
    assert_eq!(
        AGE_ADVANCES.map(|s| s.to_age),
        [
            Age::Ancient,
            Age::Feudal,
            Age::Arcane,
            Age::Industrial,
            Age::Future
        ]
    );
    for advance in &AGE_ADVANCES {
        assert_eq!(advance.duration.get(), 100);
    }
    for tech in &TECHS {
        let age = u16::from(tech.age.number());
        assert_eq!(tech.research.get(), 60);
        match tech.kind {
            TechKind::Attack => {
                assert_eq!(tech.cost_gold, 80 + 40 * age);
                assert_eq!(tech.bonus_bp.get(), 1_000);
            }
            TechKind::Armor => {
                assert_eq!(tech.cost_gold, 100 + 40 * age);
                assert_eq!(tech.bonus_bp.get(), 250);
            }
        }
    }
}

#[test]
fn spell_parameters_are_original_and_share_only_a_mechanism_family() {
    let sun = SPELLS
        .iter()
        .find(|s| s.card.id.as_str() == "sun_javelin")
        .unwrap();
    assert_eq!(
        sun.card.effect,
        SkillEffect::Pierce {
            damage: 160,
            pierce_bp: BasisPoints::new(3_000).unwrap()
        }
    );
    let chain = SPELLS
        .iter()
        .find(|s| s.card.id.as_str() == "prism_arc")
        .unwrap();
    assert_eq!(chain.card.max_targets, 4);
    assert_eq!(chain.card.cost, SkillCost::Gold(90));
    assert_eq!(chain.card.cooldown.get(), 840);
    assert_ne!(
        chain.card.effect,
        SKILLS
            .iter()
            .find(|s| s.family == SkillFamily::Chain)
            .unwrap()
            .effect
    );
}

#[test]
fn every_feature_combination_keeps_future_surfaces_closed() {
    assert_eq!(
        request_simulation(),
        Err(PhaseGateError {
            surface: DeferredSurface::Simulation
        })
    );
    assert_eq!(
        request_ai_plan(),
        Err(PhaseGateError {
            surface: DeferredSurface::AiPlanning
        })
    );
    for version in [0, 1, u16::MAX] {
        for bytes in [&[][..], &[0][..], &[255; 128][..]] {
            assert_eq!(
                request_snapshot_migration(version, bytes),
                Err(PhaseGateError {
                    surface: DeferredSurface::SnapshotMigration
                })
            );
        }
    }
}

#[test]
fn id_boundary_rejects_hostile_keys_without_normalization() {
    for raw in [
        "",
        "A",
        " guard",
        "guard ",
        "guard/other",
        "g\0uard",
        "g😀",
        "đá",
        "_guard",
    ] {
        assert_eq!(Id::new(raw), Err(SchemaError::InvalidId), "{raw:?}");
    }
    assert!(Id::new("g").is_ok());
    assert!(Id::new("age_war.burst_heat-2").is_ok());
    let limit = "a".repeat(64);
    let oversized = "a".repeat(65);
    assert!(Id::new(&limit).is_ok());
    assert_eq!(Id::new(&oversized), Err(SchemaError::InvalidId));
}

#[test]
fn numeric_boundaries_preserve_units_and_reject_overflow_domain() {
    assert_eq!(BasisPoints::new(0).unwrap().get(), 0);
    assert_eq!(BasisPoints::new(10_000).unwrap().get(), 10_000);
    assert!(BasisPoints::new(10_001).is_err());
    assert!(BasisPoints::new(u16::MAX).is_err());
    assert!(Mitigation::new(8_000).is_ok());
    assert!(Mitigation::new(8_001).is_err());
    assert!(Mitigation::new(u16::MAX).is_err());
    assert!(Q::new(MAX_Q).is_ok());
    assert!(Q::new(MAX_Q + 1).is_err());
    assert!(Q::new(u32::MAX).is_err());
    assert!(Ticks::new(MAX_TICKS).is_ok());
    assert!(Ticks::new(MAX_TICKS + 1).is_err());
    assert!(Ticks::new(u16::MAX).is_err());
}

#[test]
fn exhaustive_u16_ratio_domain_cannot_forge_above_bound() {
    for raw in 0..=u16::MAX {
        assert_eq!(BasisPoints::new(raw).is_ok(), raw <= 10_000);
        assert_eq!(Mitigation::new(raw).is_ok(), raw <= 8_000);
        assert_eq!(Ticks::new(raw).is_ok(), raw <= 24_000);
    }
}

#[test]
fn hostile_card_rejections_preserve_input_and_checked_borrow() {
    let mut raw = SKILLS[0];
    let before = raw;
    assert_eq!(raw.validate().unwrap().spec(), &before);
    raw.design_version = u16::MAX;
    let hostile = raw;
    assert_eq!(
        raw.validate().unwrap_err(),
        SchemaError::UnsupportedDesignVersion
    );
    assert_eq!(raw, hostile);
    raw = SKILLS[0];
    raw.family = SkillFamily::Root;
    assert_eq!(raw.validate().unwrap_err(), SchemaError::FamilyMismatch);
    raw = SKILLS[0];
    raw.max_targets = 0;
    assert_eq!(raw.validate().unwrap_err(), SchemaError::InvalidTargets);
    raw = SKILLS[0];
    raw.power_budget_points = 0;
    assert_eq!(
        raw.validate().unwrap_err(),
        SchemaError::MissingDocumentation
    );
}

#[test]
fn hostile_heal_and_shield_cannot_target_structures_or_drones() {
    for family in [SkillFamily::Heal, SkillFamily::Shield] {
        let original = *SKILLS.iter().find(|s| s.family == family).unwrap();
        for filter in [
            TargetFilter::UnitsAndStructures,
            TargetFilter::UnitsOnly,
            TargetFilter::SelfOnly,
        ] {
            let mut raw = original;
            raw.target_filter = filter;
            assert_eq!(raw.validate().unwrap_err(), SchemaError::InvalidTargets);
        }
        let mut raw = original;
        raw.target_side = TargetSide::Enemy;
        assert_eq!(raw.validate().unwrap_err(), SchemaError::InvalidTargets);
    }
}

#[test]
fn defensive_dispel_rejects_enemy_structure_and_drone_selectors() {
    let original = *SKILLS
        .iter()
        .find(|card| card.family == SkillFamily::Dispel)
        .unwrap();
    let mut enemy = original;
    enemy.target_side = TargetSide::Enemy;
    enemy.target_filter = TargetFilter::UnitsAndStructures;
    enemy.priority = tabula_game_age_war::schema::TargetPriority::NearestThenId;
    assert_eq!(enemy.validate().unwrap_err(), SchemaError::InvalidTargets);
    let mut drone = original;
    drone.target_filter = TargetFilter::UnitsOnly;
    assert_eq!(drone.validate().unwrap_err(), SchemaError::InvalidTargets);
    assert!(original.validate().is_ok());
}

#[test]
fn dot_cannot_replace_sources_or_reset_next_pulse() {
    let original = *SKILLS
        .iter()
        .find(|card| card.family == SkillFamily::Dot)
        .unwrap();
    let mut replacement = original;
    replacement.stacking = StackPolicy::Replace;
    replacement.refresh = tabula_game_age_war::schema::RefreshPolicy::KeepExpiry;
    assert_eq!(
        replacement.validate().unwrap_err(),
        SchemaError::InvalidStacking
    );
    let mut pulse_reset = original;
    pulse_reset.refresh = tabula_game_age_war::schema::RefreshPolicy::KeepExpiry;
    assert_eq!(
        pulse_reset.validate().unwrap_err(),
        SchemaError::InvalidStacking
    );
    assert!(original.validate().is_ok());
}

#[test]
fn basic_projectile_must_reach_declared_range_within_flight_cap() {
    let mut attack = UNITS[1].attack;
    attack.reach_q = Q::new(100_000).unwrap();
    attack.projectile_speed_q_per_tick = Q::new(1).unwrap();
    assert_eq!(attack.validate().unwrap_err(), SchemaError::InvalidAttack);
    attack.reach_q = Q::new(120).unwrap();
    assert!(attack.validate().is_ok());
    attack.reach_q = Q::new(121).unwrap();
    assert_eq!(attack.validate().unwrap_err(), SchemaError::InvalidAttack);
}

#[test]
fn hostile_control_duration_stack_and_immunity_are_rejected() {
    let mut root = *SKILLS
        .iter()
        .find(|s| s.family == SkillFamily::Root)
        .unwrap();
    root.duration = Ticks::new(13).unwrap();
    assert_eq!(root.validate().unwrap_err(), SchemaError::InvalidPayload);
    root.duration = Ticks::new(12).unwrap();
    root.immunity = Immunity::None;
    assert_eq!(root.validate().unwrap_err(), SchemaError::InvalidPayload);
    let mut slow = *SKILLS
        .iter()
        .find(|s| s.family == SkillFamily::Slow)
        .unwrap();
    slow.effect = SkillEffect::Slow {
        reduction_bp: BasisPoints::new(4_001).unwrap(),
    };
    assert_eq!(slow.validate().unwrap_err(), SchemaError::InvalidPayload);
    let mut dot = *SKILLS
        .iter()
        .find(|s| s.family == SkillFamily::Dot)
        .unwrap();
    for cap in [0, 4, u8::MAX] {
        dot.stacking = StackPolicy::PerSourceStrongest { source_cap: cap };
        assert_eq!(dot.validate().unwrap_err(), SchemaError::InvalidStacking);
    }
    let mut guard = SKILLS[0];
    guard.effect = SkillEffect::Guard {
        mitigation_bp: Mitigation::new(1_500).unwrap(),
        outgoing_penalty_bp: BasisPoints::new(2_000).unwrap(),
        hit_charges: 4,
        packet_filter: tabula_game_age_war::schema::GuardPacketFilter::DirectPhysicalAndArcane,
    };
    assert_eq!(guard.validate().unwrap_err(), SchemaError::InvalidPayload);
}

#[test]
fn hostile_payload_cardinality_and_schedules_are_rejected() {
    let mut dot = *SKILLS
        .iter()
        .find(|s| s.family == SkillFamily::Dot)
        .unwrap();
    dot.effect = SkillEffect::Dot {
        damage: 8,
        kind: tabula_game_age_war::schema::DamageKind::Arcane,
        pulses: 5,
        interval: Ticks::new(20).unwrap(),
    };
    assert_eq!(dot.validate().unwrap_err(), SchemaError::InvalidPayload);
    let mut summon = *SKILLS
        .iter()
        .find(|s| s.family == SkillFamily::Summon)
        .unwrap();
    summon.effect = SkillEffect::Summon {
        count: 3,
        drone: DRONES[0],
        lifetime: Ticks::new(240).unwrap(),
    };
    assert_eq!(summon.validate().unwrap_err(), SchemaError::InvalidPayload);
    let mut chain = *SKILLS
        .iter()
        .find(|s| s.family == SkillFamily::Chain)
        .unwrap();
    chain.max_targets = 2;
    assert_eq!(chain.validate().unwrap_err(), SchemaError::InvalidPayload);
    let mut guard = SKILLS[0];
    guard.expiry = ExpiryPolicy::ContactOnly;
    assert_eq!(guard.validate().unwrap_err(), SchemaError::InvalidTiming);
}

#[test]
fn hostile_projectile_and_commander_schedules_are_rejected() {
    let mut card = *SKILLS
        .iter()
        .find(|s| s.family == SkillFamily::Volley)
        .unwrap();
    for speed in [0, 5_001] {
        card.contact = Contact::Projectile {
            speed_q_per_tick: Q::new(speed).unwrap(),
            lifetime: Ticks::new(12).unwrap(),
        };
        assert_eq!(card.validate().unwrap_err(), SchemaError::InvalidTiming);
    }
    card.contact = Contact::Projectile {
        speed_q_per_tick: Q::new(1).unwrap(),
        lifetime: Ticks::new(120).unwrap(),
    };
    assert_eq!(card.validate().unwrap_err(), SchemaError::InvalidTiming);
    let mut spell = SPELLS[0].card;
    spell.cooldown = Ticks::new(961).unwrap();
    assert_eq!(spell.validate().unwrap_err(), SchemaError::InvalidTiming);
    spell.cooldown = Ticks::new(700).unwrap();
    spell.cost = SkillCost::Free;
    assert_eq!(spell.validate().unwrap_err(), SchemaError::InvalidTiming);
}

#[test]
fn hostile_unit_and_attack_partitions_are_rejected_without_mutation() {
    let mut unit = UNITS[0];
    unit.hp = 0;
    let hostile = unit;
    assert_eq!(unit.validate().unwrap_err(), SchemaError::ZeroStat);
    assert_eq!(unit, hostile);
    unit = UNITS[0];
    unit.population = 5;
    assert_eq!(unit.validate().unwrap_err(), SchemaError::ZeroStat);
    unit = UNITS[0];
    unit.skills = &[];
    assert_eq!(unit.validate().unwrap_err(), SchemaError::InvalidBinding);
    let mut attack = UNITS[0].attack;
    attack.damage = 0;
    assert_eq!(attack.validate().unwrap_err(), SchemaError::InvalidAttack);
    attack = UNITS[0].attack;
    attack.min_reach_q = Q::new(851).unwrap();
    assert_eq!(attack.validate().unwrap_err(), SchemaError::InvalidAttack);
    attack = UNITS[0].attack;
    attack.windup = Ticks::new(200).unwrap();
    assert_eq!(attack.validate().unwrap_err(), SchemaError::InvalidAttack);
}

#[test]
fn exact_drone_models_are_attack_only_validated_and_referenced() {
    assert_eq!(DRONES.len(), 2);
    for drone in &DRONES {
        assert!(drone.validate().is_ok());
        assert_eq!(drone.population, 1);
        assert_eq!(drone.move_q_per_tick.get(), 130);
        assert_eq!(drone.half_width_q.get(), 160);
        assert_eq!(drone.attack.reach_q.get(), 750);
        assert_eq!(drone.armor_bp.map(Mitigation::get), [0, 0, 0]);
    }
    assert_eq!(DRONES.map(|d| d.attack.period_ticks()), [30, 28]);
    for card in SKILLS.iter().chain(SPELLS.iter().map(|s| &s.card)) {
        if let SkillEffect::Summon {
            drone,
            count,
            lifetime,
        } = card.effect
        {
            assert!(DRONES.contains(&drone));
            assert_eq!(count, 2);
            assert_eq!(lifetime.get(), 240);
        }
    }
    let mut hostile = DRONES[0];
    hostile.population = 0;
    assert_eq!(hostile.validate().unwrap_err(), SchemaError::InvalidPayload);
}

#[test]
fn commander_geometry_and_projectile_lifetimes_cover_global_range() {
    for spell in &SPELLS {
        assert_eq!(spell.card.range_q.get(), 100_000);
        if spell.card.target_side == TargetSide::Ally && spell.card.max_targets > 1 {
            assert_eq!(spell.card.aoe_radius_q.get(), 3_000);
        }
    }
    let summon = SPELLS
        .iter()
        .find(|s| s.card.family == SkillFamily::Summon)
        .unwrap();
    assert_eq!(summon.card.target_side, TargetSide::Ground);
    assert_eq!(summon.card.target_filter, TargetFilter::GroundPoint);
}
