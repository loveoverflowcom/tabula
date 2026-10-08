//! Integer arithmetic oracles for the D01 proposal, not a combat reducer.
//!
//! The staged divisions are specified in `docs/games/age-war/RULES.md` R4.
//! These functions neither advance a match nor know State, timers or rendering.
//! Worked literal expectations in tests establish only this finite design-math
//! surface; gameplay conformance/TTK with moving enemies remain C01 obligations.

const BP: u64 = 10_000;
const TICKS_PER_SECOND: u64 = 20;

/// Public raw arithmetic inputs; validation is required on every call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HitInput {
    /// Positive basic packet before tech/action multipliers; zero remains zero.
    pub base_damage: u32,
    /// Aggregate researched attack bonus, capped at 3,000 bp.
    pub attack_bonus_bp: u16,
    /// Explicit card/structure action multiplier, 5,000–20,000 bp.
    pub action_multiplier_bp: u16,
    /// Mitigation for this packet's damage kind, at most 8,000 bp.
    pub armor_bp: u16,
    /// Aggregate normal-unit defense research, at most 1,500 bp.
    pub defense_bonus_bp: u16,
    /// Strongest active flat shred; not an additive status stack.
    pub shred_bp: u16,
    /// Proportion of effective armor ignored, at most 10,000 bp.
    pub pierce_bp: u16,
    /// Eligible finite guard reduction; caller selects charged packets.
    pub guard_bp: u16,
}

/// Typed rejection of invalid arithmetic domains; no panic/default success.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MathError {
    /// A declared multiplier/component lies outside the D01 domain.
    OutOfRange,
    /// A denominator/attack cycle is zero.
    ZeroDenominator,
    /// A stationary positive-HP target cannot die to zero-damage packets.
    NoDamage,
    /// Current or accumulated health/shield facts are impossible.
    InvalidHealth,
    /// No recruitment refund exists after deployment.
    AlreadyDeployed,
    /// The result cannot be represented by the promised output integer.
    Overflow,
}

/// Exact rational rate for analysis: never a float in an authoritative rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rate {
    /// Units per second numerator; denominator remains explicit.
    pub numerator: u64,
    /// Positive divisor, including period/cost/pop as applicable.
    pub denominator: u64,
}

/// Compute an individual packet with a floor at each specified stage (R4).
pub fn damage_after_modifiers(input: HitInput) -> Result<u32, MathError> {
    if input.attack_bonus_bp > 3_000
        || !(5_000..=20_000).contains(&input.action_multiplier_bp)
        || input.armor_bp > 8_000
        || input.defense_bonus_bp > 1_500
        || input.shred_bp > 8_000
        || input.pierce_bp > 10_000
        || input.guard_bp > 8_000
    {
        return Err(MathError::OutOfRange);
    }
    if input.base_damage == 0 {
        return Ok(0);
    }
    // Max intermediate <= u32::MAX * 1.3 * 20_000, safely below u64::MAX.
    let boosted = u64::from(input.base_damage) * (BP + u64::from(input.attack_bonus_bp)) / BP;
    let action = boosted * u64::from(input.action_multiplier_bp) / BP;
    let armor = (u64::from(input.armor_bp) + u64::from(input.defense_bonus_bp))
        .saturating_sub(u64::from(input.shred_bp))
        .min(8_000);
    let effective_armor = armor * (BP - u64::from(input.pierce_bp)) / BP;
    let armored = action * (BP - effective_armor) / BP;
    let guarded = armored * (BP - u64::from(input.guard_bp)) / BP;
    u32::try_from(guarded.max(1)).map_err(|_| MathError::Overflow)
}

/// Ceiling integer division with explicit zero-denominator/overflow handling.
pub fn ceil_div(numerator: u64, denominator: u64) -> Result<u64, MathError> {
    if denominator == 0 {
        return Err(MathError::ZeroDenominator);
    }
    let whole = numerator / denominator;
    whole
        .checked_add(u64::from(numerator % denominator != 0))
        .ok_or(MathError::Overflow)
}

/// Travel for a committed basic homing shot; even distance zero takes one tick.
pub fn projectile_travel_ticks(distance_q: u32, speed_q_per_tick: u32) -> Result<u64, MathError> {
    Ok(ceil_div(u64::from(distance_q), u64::from(speed_q_per_tick))?.max(1))
}

/// Steady one-target DPS, excluding whiffs, control, reload/heat and movement.
pub fn cycle_dps(damage_per_contact: u32, period_ticks: u32) -> Result<Rate, MathError> {
    if period_ticks == 0 {
        return Err(MathError::ZeroDenominator);
    }
    Ok(Rate {
        numerator: u64::from(damage_per_contact) * TICKS_PER_SECOND,
        denominator: u64::from(period_ticks),
    })
}

/// Stationary target TTK: first windup + travel + subsequent full periods.
/// Shields, heals, `DoT`, changed targets, interruptions and movement are excluded.
pub fn stationary_ttk_ticks(
    hp: u32,
    damage_per_contact: u32,
    windup_ticks: u32,
    recovery_ticks: u32,
    travel_ticks: u32,
) -> Result<u64, MathError> {
    if hp == 0 {
        return Ok(0);
    }
    if damage_per_contact == 0 {
        return Err(MathError::NoDamage);
    }
    let period = u64::from(windup_ticks) + u64::from(recovery_ticks);
    if period == 0 {
        return Err(MathError::ZeroDenominator);
    }
    let contacts = ceil_div(u64::from(hp), u64::from(damage_per_contact))?;
    let repeated = (contacts - 1)
        .checked_mul(period)
        .ok_or(MathError::Overflow)?;
    repeated
        .checked_add(u64::from(windup_ticks) + u64::from(travel_ticks))
        .ok_or(MathError::Overflow)
}

/// Continuous effective-HP ceiling for one damage kind; per-hit rounding and
/// finite guard/shield/heal budgets are deliberately outside this approximation.
pub fn effective_hp(hp: u32, mitigation_bp: u16) -> Result<u64, MathError> {
    if mitigation_bp > 8_000 {
        return Err(MathError::OutOfRange);
    }
    ceil_div(u64::from(hp) * BP, BP - u64::from(mitigation_bp))
}

/// Divide a DPS rate by a positive resource quantity (gold or population).
pub fn resource_efficiency(rate: Rate, resource: u32) -> Result<Rate, MathError> {
    if rate.denominator == 0 || resource == 0 {
        return Err(MathError::ZeroDenominator);
    }
    Ok(Rate {
        numerator: rate.numerator,
        denominator: rate
            .denominator
            .checked_mul(u64::from(resource))
            .ok_or(MathError::Overflow)?,
    })
}

/// Literal projected lifetime heal cap, bound additionally by present deficit.
/// Only a living normal-unit caller is eligible; this function rejects HP zero.
pub fn capped_heal_grant(
    max_hp: u32,
    current_hp: u32,
    prior_grants: u32,
    requested: u32,
) -> Result<u32, MathError> {
    let lifetime_cap =
        u32::try_from(u64::from(max_hp) * 3_500 / BP).map_err(|_| MathError::Overflow)?;
    if max_hp == 0 || current_hp == 0 || current_hp > max_hp || prior_grants > lifetime_cap {
        return Err(MathError::InvalidHealth);
    }
    Ok(requested
        .min(max_hp - current_hp)
        .min(lifetime_cap - prior_grants))
}

/// Projected shield grants: current pool <=25%, lifetime grants <=50% max HP.
/// Expiry may reduce the pool but never reduce `prior_grants`.
pub fn capped_shield_grant(
    max_hp: u32,
    current_pool: u32,
    prior_grants: u32,
    requested: u32,
) -> Result<u32, MathError> {
    let pool_cap =
        u32::try_from(u64::from(max_hp) * 2_500 / BP).map_err(|_| MathError::Overflow)?;
    let lifetime_cap =
        u32::try_from(u64::from(max_hp) * 5_000 / BP).map_err(|_| MathError::Overflow)?;
    if max_hp == 0
        || current_pool > pool_cap
        || prior_grants > lifetime_cap
        || current_pool > prior_grants
    {
        return Err(MathError::InvalidHealth);
    }
    Ok(requested
        .min(pool_cap - current_pool)
        .min(lifetime_cap - prior_grants))
}

/// Queue stage at acceptance, not a timer-progress guess or a mutable queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecruitmentStage {
    /// Not yet the training head; full paid gold returns.
    Waiting,
    /// Training started, including a completed head waiting for spawn space.
    Started,
    /// Already a living battlefield entity; cannot be cancelled for gold.
    Deployed,
}

/// Integer refund oracle for the R6 cancellation decision.
pub fn recruit_refund(paid_gold: u32, stage: RecruitmentStage) -> Result<u32, MathError> {
    match stage {
        RecruitmentStage::Waiting => Ok(paid_gold),
        RecruitmentStage::Started => {
            u32::try_from(u64::from(paid_gold) * 7_500 / BP).map_err(|_| MathError::Overflow)
        }
        RecruitmentStage::Deployed => Err(MathError::AlreadyDeployed),
    }
}

/// HP-adjusted turret sale refund, flooring once at the final division (R7).
pub fn turret_sale_refund(paid_gold: u32, current_hp: u32, max_hp: u32) -> Result<u32, MathError> {
    if max_hp == 0 || current_hp > max_hp {
        return Err(MathError::InvalidHealth);
    }
    let numerator = u128::from(paid_gold) * 5_000 * u128::from(current_hp);
    let denominator = u128::from(BP) * u128::from(max_hp);
    u32::try_from(numerator / denominator).map_err(|_| MathError::Overflow)
}

/// Enemy normal-unit target reward; caller owns target/source eligibility.
pub fn normal_unit_bounty(paid_gold: u32) -> u32 {
    // Euclidean splitting gives the same final floor without an overflowing
    // u32 product or a fallible conversion with a silent fallback.
    (paid_gold / 10_000) * 1_500 + ((paid_gold % 10_000) * 1_500) / 10_000
}

/// Ground AoE/body interval overlap with inclusive surface equality (R3).
/// This is an arithmetic predicate, not target acquisition or a collision solver.
pub fn overlaps_ground_area(
    center_q: u32,
    half_width_q: u32,
    impact_q: u32,
    radius_q: u32,
) -> bool {
    u64::from(center_q.abs_diff(impact_q)) <= u64::from(half_width_q) + u64::from(radius_q)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_hit() -> HitInput {
        HitInput {
            base_damage: 80,
            attack_bonus_bp: 1_000,
            action_multiplier_bp: 12_000,
            armor_bp: 2_500,
            defense_bonus_bp: 0,
            shred_bp: 0,
            pierce_bp: 0,
            guard_bp: 3_000,
        }
    }

    #[test]
    fn staged_rounding_matches_independently_worked_example() {
        assert_eq!(damage_after_modifiers(base_hit()), Ok(54));
        assert_eq!(
            damage_after_modifiers(HitInput {
                guard_bp: 0,
                ..base_hit()
            }),
            Ok(78)
        );
        assert_eq!(
            damage_after_modifiers(HitInput {
                pierce_bp: 5_000,
                ..base_hit()
            }),
            Ok(63)
        );
        assert_eq!(
            damage_after_modifiers(HitInput {
                base_damage: 1,
                attack_bonus_bp: 0,
                action_multiplier_bp: 5_000,
                armor_bp: 8_000,
                guard_bp: 8_000,
                ..base_hit()
            }),
            Ok(1)
        );
        assert_eq!(
            damage_after_modifiers(HitInput {
                base_damage: 0,
                ..base_hit()
            }),
            Ok(0)
        );
    }

    #[test]
    fn hostile_components_and_unrepresentable_output_reject() {
        for input in [
            HitInput {
                attack_bonus_bp: 3_001,
                ..base_hit()
            },
            HitInput {
                action_multiplier_bp: 4_999,
                ..base_hit()
            },
            HitInput {
                action_multiplier_bp: 20_001,
                ..base_hit()
            },
            HitInput {
                armor_bp: 8_001,
                ..base_hit()
            },
            HitInput {
                defense_bonus_bp: 1_501,
                ..base_hit()
            },
            HitInput {
                shred_bp: 8_001,
                ..base_hit()
            },
            HitInput {
                pierce_bp: 10_001,
                ..base_hit()
            },
            HitInput {
                guard_bp: 8_001,
                ..base_hit()
            },
        ] {
            assert_eq!(damage_after_modifiers(input), Err(MathError::OutOfRange));
        }
        assert_eq!(
            damage_after_modifiers(HitInput {
                base_damage: u32::MAX,
                attack_bonus_bp: 3_000,
                action_multiplier_bp: 20_000,
                armor_bp: 0,
                guard_bp: 0,
                ..base_hit()
            }),
            Err(MathError::Overflow)
        );
    }

    #[test]
    fn armor_only_domain_obeys_algebra_and_is_monotone() {
        // Exhaustive 4 x 8,001 armor domain: a simpler one-stage postcondition,
        // not the implementation's multi-stage helper used as an expectation.
        for raw in [1_u32, 17, 80, 1_000] {
            let mut previous = raw;
            for armor in 0..=8_000_u16 {
                let actual = damage_after_modifiers(HitInput {
                    base_damage: raw,
                    attack_bonus_bp: 0,
                    action_multiplier_bp: 10_000,
                    armor_bp: armor,
                    defense_bonus_bp: 0,
                    shred_bp: 0,
                    pierce_bp: 0,
                    guard_bp: 0,
                })
                .expect("enumerated valid arithmetic domain");
                let product = u64::from(raw) * u64::from(10_000 - armor);
                let lower = u64::from(actual.max(1)) * BP;
                if product >= BP {
                    assert!(lower <= product && product < lower + BP);
                } else {
                    assert_eq!(actual, 1);
                }
                assert!(actual <= previous);
                previous = actual;
            }
        }
    }

    #[test]
    fn widening_and_ceiling_cover_integer_extremes() {
        assert_eq!(ceil_div(0, 9), Ok(0));
        assert_eq!(ceil_div(5_000, 2_000), Ok(3));
        assert_eq!(ceil_div(u64::MAX, u64::MAX), Ok(1));
        assert_eq!(ceil_div(u64::MAX, 1), Ok(u64::MAX));
        assert_eq!(ceil_div(1, 0), Err(MathError::ZeroDenominator));
        assert_eq!(projectile_travel_ticks(0, 1), Ok(1));
        assert_eq!(projectile_travel_ticks(5_000, 2_000), Ok(3));
        assert_eq!(
            projectile_travel_ticks(5_000, 0),
            Err(MathError::ZeroDenominator)
        );
    }

    #[test]
    fn dps_ttk_and_efficiency_are_exact_under_stated_assumptions() {
        let rate = Rate {
            numerator: 1_080,
            denominator: 20,
        };
        assert_eq!(cycle_dps(54, 20), Ok(rate));
        assert_eq!(stationary_ttk_ticks(300, 54, 4, 16, 3), Ok(107));
        assert_eq!(stationary_ttk_ticks(54, 54, 4, 16, 3), Ok(7));
        assert_eq!(stationary_ttk_ticks(0, 0, 0, 0, 0), Ok(0));
        assert_eq!(effective_hp(300, 2_500), Ok(400));
        assert_eq!(
            resource_efficiency(rate, 80),
            Ok(Rate {
                numerator: 1_080,
                denominator: 1_600
            })
        );
        assert_eq!(
            resource_efficiency(rate, 2),
            Ok(Rate {
                numerator: 1_080,
                denominator: 40
            })
        );
        assert_eq!(cycle_dps(54, 0), Err(MathError::ZeroDenominator));
        assert_eq!(
            stationary_ttk_ticks(300, 0, 4, 16, 3),
            Err(MathError::NoDamage)
        );
        assert_eq!(
            stationary_ttk_ticks(300, 54, 0, 0, 3),
            Err(MathError::ZeroDenominator)
        );
        assert_eq!(effective_hp(300, 8_001), Err(MathError::OutOfRange));
        assert_eq!(
            resource_efficiency(rate, 0),
            Err(MathError::ZeroDenominator)
        );
        assert_eq!(
            resource_efficiency(
                Rate {
                    numerator: 1,
                    denominator: u64::MAX
                },
                2
            ),
            Err(MathError::Overflow)
        );
    }

    #[test]
    fn heal_and_shield_caps_do_not_reset_after_expiry() {
        assert_eq!(capped_heal_grant(200, 100, 65, 40), Ok(5));
        assert_eq!(capped_heal_grant(200, 199, 0, 40), Ok(1));
        assert_eq!(capped_heal_grant(200, 100, 70, 40), Ok(0));
        assert_eq!(
            capped_heal_grant(200, 0, 0, 40),
            Err(MathError::InvalidHealth)
        );
        assert_eq!(
            capped_heal_grant(200, 201, 0, 40),
            Err(MathError::InvalidHealth)
        );
        assert_eq!(capped_shield_grant(200, 20, 90, 40), Ok(10));
        assert_eq!(capped_shield_grant(200, 0, 100, 40), Ok(0));
        assert_eq!(
            capped_shield_grant(200, 51, 90, 40),
            Err(MathError::InvalidHealth)
        );
        assert_eq!(
            capped_shield_grant(200, 40, 20, 40),
            Err(MathError::InvalidHealth)
        );
    }

    #[test]
    fn refunds_floor_at_the_documented_stage_and_do_not_create_gold() {
        assert_eq!(recruit_refund(101, RecruitmentStage::Waiting), Ok(101));
        assert_eq!(recruit_refund(101, RecruitmentStage::Started), Ok(75));
        assert_eq!(
            recruit_refund(101, RecruitmentStage::Deployed),
            Err(MathError::AlreadyDeployed)
        );
        assert_eq!(turret_sale_refund(101, 50, 100), Ok(25));
        assert_eq!(turret_sale_refund(101, 100, 100), Ok(50));
        assert_eq!(turret_sale_refund(101, 1, 3), Ok(16));
        assert_eq!(
            turret_sale_refund(u32::MAX, u32::MAX, u32::MAX),
            Ok(u32::MAX / 2)
        );
        assert_eq!(
            turret_sale_refund(101, 101, 100),
            Err(MathError::InvalidHealth)
        );
        assert_eq!(turret_sale_refund(101, 1, 0), Err(MathError::InvalidHealth));
        assert_eq!(normal_unit_bounty(80), 12);
        assert_eq!(normal_unit_bounty(101), 15);
        for paid in 0..=10_000 {
            assert!(normal_unit_bounty(paid) <= paid);
            assert!(recruit_refund(paid, RecruitmentStage::Started).expect("valid stage") <= paid);
        }
    }

    #[test]
    fn inclusive_ground_occupancy_uses_surface_not_visual_center() {
        assert!(overlaps_ground_area(10_000, 500, 8_000, 1_500));
        assert!(!overlaps_ground_area(10_001, 500, 8_000, 1_500));
        assert!(overlaps_ground_area(0, u32::MAX, u32::MAX, u32::MAX));
    }
}
