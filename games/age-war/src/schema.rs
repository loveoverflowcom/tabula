//! D01 typed design descriptors and explicit raw-to-validated boundaries.
//!
//! These are not canonical gameplay/wire types. Numeric newtypes prevent unit
//! confusion; cross-field validation establishes only descriptor coherence,
//! not game balance or simulator correctness (doc 02 §3, I-2/I-16).

use std::fmt;

/// One logical simulation quantum in the proposed C01, in milliseconds.
pub const TICK_MILLIS: u16 = 50;
/// Integer coordinate scale: one world unit is this many q.
pub const Q_PER_WORLD_UNIT: u32 = 1_000;
/// D01 authored coordinate/distance bound; Cast point still needs its separate legal-area check.
pub const MAX_Q: u32 = 100_000;
/// Twenty-minute proposed hard limit, expressed as logical ticks.
pub const MAX_TICKS: u16 = 24_000;
/// Normalized ratio denominator; 10,000 bp is 100%.
pub const BP_ONE: u16 = 10_000;
/// Armor composition/component ceiling; finite guard is a separate R4 stage.
pub const MAX_MITIGATION_BP: u16 = 8_000;

/// Stable authored key. No mutable, default or deserialization bypass exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Id<'a>(&'a str);

impl<'a> Id<'a> {
    /// Validate a borrowed ASCII key without allocation or normalization.
    pub fn new(value: &'a str) -> Result<Self, SchemaError> {
        if value.is_empty() || value.len() > 64 {
            return Err(SchemaError::InvalidId);
        }
        if !value.as_bytes()[0].is_ascii_lowercase()
            || !value.bytes().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'_' | b'-' | b'.')
            })
        {
            return Err(SchemaError::InvalidId);
        }
        Ok(Self(value))
    }

    /// Read the exact source key; no case folding or silent aliasing.
    pub const fn as_str(self) -> &'a str {
        self.0
    }

    /// Trusted authored literal only; every producer is checked by catalog tests.
    pub(crate) const fn catalog_literal(value: &'a str) -> Self {
        Self(value)
    }
}

macro_rules! bounded_unit {
    ($name:ident, $inner:ty, $bound:expr, $description:literal) => {
        #[doc = $description]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
        pub struct $name($inner);
        impl $name {
            /// Validate the intrinsic upper bound. Zero is meaningful for some fields.
            pub const fn new(value: $inner) -> Result<Self, SchemaError> {
                if value > $bound {
                    Err(SchemaError::OutOfRange)
                } else {
                    Ok(Self(value))
                }
            }
            /// Read the scaled integer; arithmetic belongs to a separate pure function.
            pub const fn get(self) -> $inner {
                self.0
            }
            /// Trusted authored literal; callers must validate each containing descriptor.
            pub(crate) const fn catalog_literal(value: $inner) -> Self {
                Self(value)
            }
        }
    };
}

bounded_unit!(
    Ticks,
    u16,
    MAX_TICKS,
    "Logical 50 ms ticks; bounded by the D01 hard match limit."
);
bounded_unit!(
    Q,
    u32,
    MAX_Q,
    "Unsigned authored q coordinate/distance, not a screen pixel or unrestricted signed position."
);
bounded_unit!(
    BasisPoints,
    u16,
    BP_ONE,
    "A ratio in [0, 10,000]; bonus ratios are added to unity separately."
);
bounded_unit!(
    Mitigation,
    u16,
    MAX_MITIGATION_BP,
    "One mitigation component in [0, 8,000] bp; final composition is also capped."
);

/// The issue's six age identities, in advancement order; labels are presentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Age {
    Primitive,
    Ancient,
    Feudal,
    Arcane,
    Industrial,
    Future,
}

impl Age {
    /// Exhaustive stable order used by coverage and catalog-reference tests.
    pub const ALL: [Self; 6] = [
        Self::Primitive,
        Self::Ancient,
        Self::Feudal,
        Self::Arcane,
        Self::Industrial,
        Self::Future,
    ];
    /// Human-friendly one-based age index; no unchecked integer-to-age conversion.
    pub const fn number(self) -> u8 {
        match self {
            Self::Primitive => 1,
            Self::Ancient => 2,
            Self::Feudal => 3,
            Self::Arcane => 4,
            Self::Industrial => 5,
            Self::Future => 6,
        }
    }
    /// Issue-mandated Vietnamese label; not an i18n runtime implementation.
    pub const fn label_vi(self) -> &'static str {
        match self {
            Self::Primitive => "Hoang sơ",
            Self::Ancient => "Cổ đại",
            Self::Feudal => "Phong kiến",
            Self::Arcane => "Huyền thuật",
            Self::Industrial => "Công nghiệp",
            Self::Future => "Tương lai",
        }
    }
}

/// Six roster roles; each age contains exactly one of each in this draft.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum UnitRole {
    Frontline,
    Ranged,
    Tank,
    Skirmisher,
    Support,
    Siege,
}

impl UnitRole {
    /// Stable role inventory for per-age completeness checks.
    pub const ALL: [Self; 6] = [
        Self::Frontline,
        Self::Ranged,
        Self::Tank,
        Self::Skirmisher,
        Self::Support,
        Self::Siege,
    ];
}

/// Typed mitigation channel; there is no unbounded true-damage channel in D01.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DamageKind {
    Physical,
    Arcane,
    Siege,
}

/// Noncanonical D02 art hint only. Never a combat, target or balance selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentationHint {
    FeminineHuman,
    MasculineHuman,
    Construct,
}

/// Reusable mechanic identity. Spell and turret cards reuse these families.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SkillFamily {
    Guard,
    Pierce,
    Shred,
    Charge,
    Volley,
    Dot,
    Slow,
    Root,
    Knockback,
    Heal,
    Shield,
    Dispel,
    Chain,
    Bombard,
    Summon,
    BurstHeat,
}

impl SkillFamily {
    /// Exhaustive mechanism inventory; exactly sixteen, never age reskins.
    pub const ALL: [Self; 16] = [
        Self::Guard,
        Self::Pierce,
        Self::Shred,
        Self::Charge,
        Self::Volley,
        Self::Dot,
        Self::Slow,
        Self::Root,
        Self::Knockback,
        Self::Heal,
        Self::Shield,
        Self::Dispel,
        Self::Chain,
        Self::Bombard,
        Self::Summon,
        Self::BurstHeat,
    ];
    /// Stable source key for docs, bindings and future versioned adapters.
    pub const fn key(self) -> &'static str {
        match self {
            Self::Guard => "guard",
            Self::Pierce => "pierce",
            Self::Shred => "shred",
            Self::Charge => "charge",
            Self::Volley => "volley",
            Self::Dot => "dot",
            Self::Slow => "slow",
            Self::Root => "root",
            Self::Knockback => "knockback",
            Self::Heal => "heal",
            Self::Shield => "shield",
            Self::Dispel => "dispel",
            Self::Chain => "chain",
            Self::Bombard => "bombard",
            Self::Summon => "summon",
            Self::BurstHeat => "burst_heat",
        }
    }
}

/// Owner of activation; all are descriptors, no automatic execution exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Activation {
    Passive,
    OnAttackContact,
    AutoCooldown,
    CommanderCast,
}
/// Additional activation condition after selector eligibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    Always,
    AttackContact,
    UninterruptedRun,
    MissingHp,
    HasDispellableStatus,
    HeatBudget,
}
/// Relation to the source for target acquisition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetSide {
    SelfOnly,
    Ally,
    Enemy,
    Ground,
}
/// Exact entity class filter; regular units exclude drones and structures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetFilter {
    SelfOnly,
    NormalUnits,
    UnitsAndStructures,
    UnitsOnly,
    GroundPoint,
}
/// Deterministic priority; every tie ends at ascending stable entity id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetPriority {
    SelfOnly,
    NearestThenId,
    LowestHpRatioThenNearestThenId,
    ThreatThenHpRatioThenNearestThenId,
    HostileControlThenOldestThenId,
    GroundPoint,
}
/// Resource ownership/cost is explicit; no hidden skill-energy recharge exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillCost {
    Free,
    Gold(u16),
}
/// Travel/contact schedule after windup; all intervals are logical ticks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Contact {
    Instant,
    Projectile {
        speed_q_per_tick: Q,
        lifetime: Ticks,
    },
    DelayedArea {
        delay: Ticks,
    },
}
/// Status instances per (family, target); source ids determine tied replacement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackPolicy {
    UniqueStrongest,
    PerSourceStrongest { source_cap: u8 },
    Replace,
}
/// Refreshing a same-family effect cannot bypass its cap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshPolicy {
    KeepExpiry,
    ExtendExpiry,
    ExtendExpiryKeepPulse,
}
/// Exact expiry schedule; source death never grants extra elapsed ticks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpiryPolicy {
    ContactOnly,
    Timeout,
    NextAttackOrTimeout,
    ChargesSpentOrTimeout,
}
/// Which status class a cleanse may remove; instant damage cannot be undone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispelTag {
    None,
    Positive,
    Negative,
}
/// Hard-control protection after contact; structures are intrinsically protected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Immunity {
    None,
    Control { after_ticks: Ticks },
    Structures,
}
/// Future interrupt semantics; interrupts never refund a committed commander cast.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterruptPolicy {
    WindupOnHardControl,
    Never,
}

/// Renderer-independent keys only, following I-10; no assets are shipped in D01.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CueKeys {
    pub activate: Id<'static>,
    pub contact: Id<'static>,
    pub expire: Id<'static>,
    pub audio: Id<'static>,
}

/// Packet eligibility for finite guard charges; `DoT` and Siege are excluded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuardPacketFilter {
    DirectPhysicalAndArcane,
}

/// Exact reusable payload; numeric effect semantics are specified in CONTENT.md.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillEffect {
    Guard {
        mitigation_bp: Mitigation,
        outgoing_penalty_bp: BasisPoints,
        hit_charges: u8,
        packet_filter: GuardPacketFilter,
    },
    Pierce {
        damage: u16,
        pierce_bp: BasisPoints,
    },
    Shred {
        mitigation_loss_bp: Mitigation,
    },
    Charge {
        bonus_damage: u16,
        required_run_q: Q,
    },
    Volley {
        damage: u16,
        kind: DamageKind,
        shots: u8,
        spacing: Ticks,
    },
    Dot {
        damage: u16,
        kind: DamageKind,
        pulses: u8,
        interval: Ticks,
    },
    Slow {
        reduction_bp: BasisPoints,
    },
    Root,
    Knockback {
        distance_q: Q,
    },
    Heal {
        hp: u16,
    },
    Shield {
        hp: u16,
    },
    Dispel {
        remove_up_to: u8,
    },
    Chain {
        damage: u16,
        kind: DamageKind,
        jumps: u8,
        jump_range_q: Q,
        retention_bp: BasisPoints,
    },
    Bombard {
        damage: u16,
        kind: DamageKind,
    },
    Summon {
        count: u8,
        drone: DroneSpec,
        lifetime: Ticks,
    },
    BurstHeat {
        damage: u16,
        kind: DamageKind,
        shots: u8,
        spacing: Ticks,
        heat_per_shot: u8,
        heat_limit: u8,
        decay_per_tick: u8,
    },
}

impl SkillEffect {
    /// Payload mechanism; mismatch against a declared family is invalid.
    pub const fn family(self) -> SkillFamily {
        match self {
            Self::Guard { .. } => SkillFamily::Guard,
            Self::Pierce { .. } => SkillFamily::Pierce,
            Self::Shred { .. } => SkillFamily::Shred,
            Self::Charge { .. } => SkillFamily::Charge,
            Self::Volley { .. } => SkillFamily::Volley,
            Self::Dot { .. } => SkillFamily::Dot,
            Self::Slow { .. } => SkillFamily::Slow,
            Self::Root => SkillFamily::Root,
            Self::Knockback { .. } => SkillFamily::Knockback,
            Self::Heal { .. } => SkillFamily::Heal,
            Self::Shield { .. } => SkillFamily::Shield,
            Self::Dispel { .. } => SkillFamily::Dispel,
            Self::Chain { .. } => SkillFamily::Chain,
            Self::Bombard { .. } => SkillFamily::Bombard,
            Self::Summon { .. } => SkillFamily::Summon,
            Self::BurstHeat { .. } => SkillFamily::BurstHeat,
        }
    }
}

/// Full raw authored skill card. Public fields support hostile descriptor tests;
/// use `validate` before trusting cross-field coherence. No serde/loader exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkillCard {
    pub id: Id<'static>,
    pub design_version: u16,
    pub family: SkillFamily,
    pub activation: Activation,
    pub trigger: Trigger,
    pub target_side: TargetSide,
    pub target_filter: TargetFilter,
    pub priority: TargetPriority,
    pub cost: SkillCost,
    pub range_q: Q,
    pub aoe_radius_q: Q,
    pub max_targets: u8,
    pub windup: Ticks,
    pub contact: Contact,
    pub recovery: Ticks,
    pub cooldown: Ticks,
    pub duration: Ticks,
    pub stacking: StackPolicy,
    pub refresh: RefreshPolicy,
    pub expiry: ExpiryPolicy,
    pub dispel: DispelTag,
    pub immunity: Immunity,
    pub interrupt: InterruptPolicy,
    pub effect: SkillEffect,
    pub cues: CueKeys,
    pub power_budget_points: u16,
    pub counterplay_vi: &'static str,
    pub oracle_vi: &'static str,
}

/// A coherent borrowed descriptor; it cannot outlive or mutate its raw source.
#[derive(Debug, Clone, Copy)]
pub struct Validated<'a, T> {
    spec: &'a T,
}
impl<'a, T> Validated<'a, T> {
    /// Borrow the checked descriptor; no unchecked mutable access is exposed.
    pub const fn spec(&self) -> &'a T {
        self.spec
    }
}

/// Stable descriptor rejection classes; no mutation occurs during validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaError {
    InvalidId,
    OutOfRange,
    UnsupportedDesignVersion,
    ZeroStat,
    InvalidAttack,
    InvalidTargets,
    InvalidTiming,
    FamilyMismatch,
    InvalidStacking,
    InvalidControl,
    InvalidPayload,
    MissingDocumentation,
    InvalidBinding,
}
impl fmt::Display for SchemaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid D01 descriptor: {self:?}")
    }
}
impl std::error::Error for SchemaError {}

impl SkillCard {
    /// Establish descriptor-local coherence only, never gameplay availability.
    pub fn validate(&self) -> Result<Validated<'_, Self>, SchemaError> {
        Id::new(self.id.as_str())?;
        if self.design_version != 1 {
            return Err(SchemaError::UnsupportedDesignVersion);
        }
        if self.family != self.effect.family() {
            return Err(SchemaError::FamilyMismatch);
        }
        Q::new(self.range_q.get())?;
        Q::new(self.aoe_radius_q.get())?;
        for ticks in [self.windup, self.recovery, self.cooldown, self.duration] {
            Ticks::new(ticks.get())?;
        }
        if !(1..=8).contains(&self.max_targets) {
            return Err(SchemaError::InvalidTargets);
        }
        self.validate_selector()?;
        self.validate_timing()?;
        self.validate_effect()?;
        for key in [
            self.cues.activate,
            self.cues.contact,
            self.cues.expire,
            self.cues.audio,
        ] {
            Id::new(key.as_str())?;
        }
        if self.power_budget_points == 0
            || self.power_budget_points > 1_000
            || self.counterplay_vi.trim().is_empty()
            || self.oracle_vi.trim().is_empty()
        {
            return Err(SchemaError::MissingDocumentation);
        }
        Ok(Validated { spec: self })
    }

    fn validate_selector(&self) -> Result<(), SchemaError> {
        let selector_valid = match self.target_side {
            TargetSide::SelfOnly => {
                self.target_filter == TargetFilter::SelfOnly
                    && self.priority == TargetPriority::SelfOnly
                    && self.range_q.get() == 0
                    && self.max_targets == 1
            }
            TargetSide::Ally => {
                self.target_filter == TargetFilter::NormalUnits
                    && matches!(
                        self.priority,
                        TargetPriority::NearestThenId
                            | TargetPriority::LowestHpRatioThenNearestThenId
                            | TargetPriority::ThreatThenHpRatioThenNearestThenId
                            | TargetPriority::HostileControlThenOldestThenId
                    )
                    && self.range_q.get() > 0
            }
            TargetSide::Enemy => {
                matches!(
                    self.target_filter,
                    TargetFilter::NormalUnits
                        | TargetFilter::UnitsAndStructures
                        | TargetFilter::UnitsOnly
                ) && self.priority == TargetPriority::NearestThenId
                    && self.range_q.get() > 0
            }
            TargetSide::Ground => {
                self.target_filter == TargetFilter::GroundPoint
                    && self.priority == TargetPriority::GroundPoint
                    && self.range_q.get() > 0
            }
        };
        if !selector_valid {
            return Err(SchemaError::InvalidTargets);
        }
        if matches!(
            self.family,
            SkillFamily::Heal | SkillFamily::Shield | SkillFamily::Dispel
        ) && (self.target_side != TargetSide::Ally
            || self.target_filter != TargetFilter::NormalUnits)
        {
            return Err(SchemaError::InvalidTargets);
        }
        if self.target_side == TargetSide::Ground
            && (self.target_filter != TargetFilter::GroundPoint
                || self.priority != TargetPriority::GroundPoint)
        {
            return Err(SchemaError::InvalidTargets);
        }
        Ok(())
    }

    fn validate_timing(&self) -> Result<(), SchemaError> {
        if self.family == SkillFamily::Dot
            && (self.stacking != StackPolicy::PerSourceStrongest { source_cap: 3 }
                || self.refresh != RefreshPolicy::ExtendExpiryKeepPulse
                || self.expiry != ExpiryPolicy::Timeout)
        {
            return Err(SchemaError::InvalidStacking);
        }
        if let StackPolicy::PerSourceStrongest { source_cap } = self.stacking {
            if source_cap != 3 || self.family != SkillFamily::Dot {
                return Err(SchemaError::InvalidStacking);
            }
        }
        if (self.duration.get() == 0) != (self.expiry == ExpiryPolicy::ContactOnly) {
            return Err(SchemaError::InvalidTiming);
        }
        if self.activation == Activation::CommanderCast {
            if self.cooldown.get() == 0
                || self.cooldown.get() > 960
                || !matches!(self.cost, SkillCost::Gold(1..=200))
            {
                return Err(SchemaError::InvalidTiming);
            }
        } else if self.cost != SkillCost::Free {
            return Err(SchemaError::InvalidPayload);
        }
        match self.contact {
            Contact::Projectile {
                speed_q_per_tick,
                lifetime,
            } => {
                if speed_q_per_tick.get() == 0
                    || speed_q_per_tick.get() > 5_000
                    || lifetime.get() == 0
                    || lifetime.get() > 120
                    || u64::from(speed_q_per_tick.get()) * u64::from(lifetime.get())
                        < u64::from(self.range_q.get())
                {
                    return Err(SchemaError::InvalidTiming);
                }
            }
            Contact::DelayedArea { delay } if delay.get() == 0 || delay.get() > 60 => {
                return Err(SchemaError::InvalidTiming)
            }
            Contact::Instant | Contact::DelayedArea { .. } => {}
        }
        if matches!(
            self.family,
            SkillFamily::Guard
                | SkillFamily::Shred
                | SkillFamily::Charge
                | SkillFamily::Dot
                | SkillFamily::Slow
                | SkillFamily::Shield
                | SkillFamily::Summon
        ) && self.duration.get() == 0
        {
            return Err(SchemaError::InvalidTiming);
        }
        Ok(())
    }

    fn validate_effect(&self) -> Result<(), SchemaError> {
        let positive_damage = |damage: u16| damage > 0 && damage <= 2_000;
        let good = match self.effect {
            SkillEffect::Guard {
                mitigation_bp,
                outgoing_penalty_bp,
                hit_charges,
                ..
            } => {
                Mitigation::new(mitigation_bp.get()).is_ok()
                    && outgoing_penalty_bp.get() == 2_000
                    && (1..=3).contains(&hit_charges)
                    && self.expiry == ExpiryPolicy::ChargesSpentOrTimeout
            }
            SkillEffect::Pierce { damage, pierce_bp } => {
                damage <= 2_000 && BasisPoints::new(pierce_bp.get()).is_ok()
            }
            SkillEffect::Shred { mitigation_loss_bp } => {
                mitigation_loss_bp.get() > 0 && Mitigation::new(mitigation_loss_bp.get()).is_ok()
            }
            SkillEffect::Charge {
                bonus_damage,
                required_run_q,
            } => positive_damage(bonus_damage) && Q::new(required_run_q.get()).is_ok(),
            SkillEffect::Slow { reduction_bp } => {
                reduction_bp.get() > 0 && reduction_bp.get() <= 4_000
            }
            SkillEffect::Root => {
                self.duration.get() > 0
                    && self.duration.get() <= 12
                    && self.immunity
                        == Immunity::Control {
                            after_ticks: Ticks::catalog_literal(40),
                        }
                    && self.target_filter == TargetFilter::NormalUnits
            }
            SkillEffect::Knockback { distance_q } => {
                distance_q.get() > 0
                    && distance_q.get() <= 2_000
                    && self.immunity
                        == Immunity::Control {
                            after_ticks: Ticks::catalog_literal(40),
                        }
                    && self.target_filter == TargetFilter::NormalUnits
            }
            SkillEffect::Heal { hp } | SkillEffect::Shield { hp } => positive_damage(hp),
            SkillEffect::Dispel { remove_up_to } => (1..=3).contains(&remove_up_to),
            SkillEffect::Bombard { damage, .. } => {
                positive_damage(damage) && self.aoe_radius_q.get() > 0
            }
            SkillEffect::Volley { .. }
            | SkillEffect::Dot { .. }
            | SkillEffect::Chain { .. }
            | SkillEffect::Summon { .. }
            | SkillEffect::BurstHeat { .. } => self.scheduled_effect_is_coherent(),
        };
        if good {
            Ok(())
        } else {
            Err(SchemaError::InvalidPayload)
        }
    }
    fn scheduled_effect_is_coherent(&self) -> bool {
        let positive_damage = |damage: u16| damage > 0 && damage <= 2_000;
        match self.effect {
            SkillEffect::Volley {
                damage,
                shots,
                spacing,
                ..
            } => {
                positive_damage(damage)
                    && (2..=4).contains(&shots)
                    && spacing.get() > 0
                    && spacing.get() <= 20
            }
            SkillEffect::Dot {
                damage,
                pulses,
                interval,
                ..
            } => {
                positive_damage(damage)
                    && (1..=6).contains(&pulses)
                    && interval.get() > 0
                    && u32::from(pulses) * u32::from(interval.get())
                        < u32::from(self.duration.get())
            }
            SkillEffect::Chain {
                damage,
                jumps,
                jump_range_q,
                retention_bp,
                ..
            } => {
                positive_damage(damage)
                    && (2..=4).contains(&jumps)
                    && jumps <= self.max_targets
                    && jump_range_q.get() > 0
                    && jump_range_q.get() <= 3_000
                    && retention_bp.get() > 0
                    && BasisPoints::new(retention_bp.get()).is_ok()
            }
            SkillEffect::Summon {
                count,
                drone,
                lifetime,
            } => {
                count == 2
                    && drone.validate().is_ok()
                    && lifetime.get() == 240
                    && self.duration == lifetime
                    && self.expiry == ExpiryPolicy::Timeout
            }
            SkillEffect::BurstHeat {
                damage,
                shots,
                spacing,
                heat_per_shot,
                heat_limit,
                decay_per_tick,
                ..
            } => {
                positive_damage(damage)
                    && (2..=4).contains(&shots)
                    && spacing.get() > 0
                    && spacing.get() <= 20
                    && heat_limit == 100
                    && heat_per_shot > 0
                    && u16::from(shots) * u16::from(heat_per_shot) <= u16::from(heat_limit)
                    && decay_per_tick > 0
                    && decay_per_tick <= 5
            }
            _ => false,
        }
    }
}

/// Basic attack data, distinct from any skill activation schedule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttackSpec {
    pub damage: u16,
    pub kind: DamageKind,
    pub windup: Ticks,
    pub recovery: Ticks,
    pub reach_q: Q,
    pub min_reach_q: Q,
    /// Zero means melee contact; nonzero means one projectile, TTL <=120 ticks.
    pub projectile_speed_q_per_tick: Q,
}
impl AttackSpec {
    /// Attack period is windup + recovery; no float or concealed attack speed.
    pub const fn period_ticks(self) -> u32 {
        self.windup.get() as u32 + self.recovery.get() as u32
    }
    /// Check positivity, nonzero period, ordered range, and bounded projectile speed.
    pub fn validate(&self) -> Result<Validated<'_, Self>, SchemaError> {
        if self.damage == 0
            || self.damage > 2_000
            || self.windup.get() == 0
            || self.recovery.get() == 0
            || self.period_ticks() > 200
            || self.reach_q.get() == 0
            || self.min_reach_q.get() > self.reach_q.get()
            || self.projectile_speed_q_per_tick.get() > 5_000
        {
            return Err(SchemaError::InvalidAttack);
        }
        Q::new(self.reach_q.get())?;
        Q::new(self.min_reach_q.get())?;
        let speed = self.projectile_speed_q_per_tick.get();
        if speed != 0 && u64::from(speed) * 120 < u64::from(self.reach_q.get()) {
            return Err(SchemaError::InvalidAttack);
        }
        Ok(Validated { spec: self })
    }
}

/// Exact attack-only drone model; no heal/shield/skill/tech capability fields exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DroneSpec {
    pub id: Id<'static>,
    pub design_version: u16,
    pub hp: u16,
    pub population: u8,
    pub armor_bp: [Mitigation; 3],
    pub move_q_per_tick: Q,
    pub half_width_q: Q,
    pub attack: AttackSpec,
}
impl DroneSpec {
    /// Check every spawn/attack statistic; no defaults come from the summoning unit.
    pub fn validate(&self) -> Result<Validated<'_, Self>, SchemaError> {
        Id::new(self.id.as_str())?;
        if self.design_version != 1 {
            return Err(SchemaError::UnsupportedDesignVersion);
        }
        if self.hp == 0
            || self.hp > 2_000
            || self.population != 1
            || self.move_q_per_tick.get() == 0
            || self.move_q_per_tick.get() > 250
            || !(100..=1_000).contains(&self.half_width_q.get())
            || self.attack.kind != DamageKind::Physical
        {
            return Err(SchemaError::InvalidPayload);
        }
        for armor in self.armor_bp {
            Mitigation::new(armor.get())?;
        }
        self.attack.validate()?;
        Ok(Validated { spec: self })
    }
}

/// Exact versioned reference, resolved against SKILLS by catalog tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkillBinding {
    pub id: Id<'static>,
    pub design_version: u16,
}

/// Original raw unit descriptor; D02 presentation hints are never combat facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnitSpec {
    pub id: Id<'static>,
    pub design_version: u16,
    pub age: Age,
    pub name_vi: &'static str,
    pub role: UnitRole,
    pub presentation_hint: PresentationHint,
    pub cost_gold: u16,
    pub population: u8,
    pub train: Ticks,
    pub hp: u16,
    /// Physical, arcane, siege in that exact order.
    pub armor_bp: [Mitigation; 3],
    pub move_q_per_tick: Q,
    pub half_width_q: Q,
    pub attack: AttackSpec,
    pub skills: &'static [SkillBinding],
    pub counterplay_vi: &'static str,
}
impl UnitSpec {
    /// Check raw stats and binding syntax, preserving the raw input on failure.
    pub fn validate(&self) -> Result<Validated<'_, Self>, SchemaError> {
        Id::new(self.id.as_str())?;
        if self.design_version != 1 {
            return Err(SchemaError::UnsupportedDesignVersion);
        }
        if self.cost_gold == 0
            || self.cost_gold > 500
            || !(1..=4).contains(&self.population)
            || self.train.get() == 0
            || self.train.get() > 200
            || self.hp == 0
            || self.hp > 10_000
            || self.move_q_per_tick.get() == 0
            || self.move_q_per_tick.get() > 250
            || !(100..=1_000).contains(&self.half_width_q.get())
        {
            return Err(SchemaError::ZeroStat);
        }
        for armor in self.armor_bp {
            Mitigation::new(armor.get())?;
        }
        self.attack.validate()?;
        if self.skills.is_empty() || self.skills.len() > 2 {
            return Err(SchemaError::InvalidBinding);
        }
        for (index, binding) in self.skills.iter().enumerate() {
            Id::new(binding.id.as_str())?;
            if binding.design_version != 1
                || self.skills[..index]
                    .iter()
                    .any(|prior| prior.id == binding.id)
            {
                return Err(SchemaError::InvalidBinding);
            }
        }
        if self.name_vi.trim().is_empty() || self.counterplay_vi.trim().is_empty() {
            return Err(SchemaError::MissingDocumentation);
        }
        Ok(Validated { spec: self })
    }
}

/// One of two persistent commander slots; there is no third-slot representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SpellSlot {
    First,
    Second,
}
/// Age-specific commander card; exact parameters reside in the embedded full card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpellSpec {
    pub age: Age,
    pub slot: SpellSlot,
    pub name_vi: &'static str,
    pub card: SkillCard,
}
/// Mutually exclusive turret branch selected per physical slot; two slots/owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TurretBranch {
    Direct,
    Control,
}
/// Stationary attack descriptor; no population, training, healing or shield fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TurretSpec {
    pub id: Id<'static>,
    pub design_version: u16,
    pub age: Age,
    pub branch: TurretBranch,
    pub name_vi: &'static str,
    pub cost_gold: u16,
    pub build: Ticks,
    pub hp: u16,
    pub armor_bp: [Mitigation; 3],
    pub attack: AttackSpec,
    pub skills: &'static [SkillBinding],
}
impl TurretSpec {
    /// Check stationary raw descriptor and reference syntax; no live turret exists.
    pub fn validate(&self) -> Result<Validated<'_, Self>, SchemaError> {
        Id::new(self.id.as_str())?;
        if self.design_version != 1 {
            return Err(SchemaError::UnsupportedDesignVersion);
        }
        if self.cost_gold == 0
            || self.cost_gold > 1_000
            || self.hp == 0
            || self.hp > 10_000
            || self.build.get() == 0
            || self.build.get() > 200
            || self.skills.len() != 1
        {
            return Err(SchemaError::ZeroStat);
        }
        for armor in self.armor_bp {
            Mitigation::new(armor.get())?;
        }
        self.attack.validate()?;
        for (index, binding) in self.skills.iter().enumerate() {
            Id::new(binding.id.as_str())?;
            if binding.design_version != 1
                || self.skills[..index]
                    .iter()
                    .any(|prior| prior.id == binding.id)
            {
                return Err(SchemaError::InvalidBinding);
            }
        }
        if self.name_vi.trim().is_empty() {
            return Err(SchemaError::MissingDocumentation);
        }
        Ok(Validated { spec: self })
    }
}
/// Nonstackable technology kind. Aggregate caps are a separate future reducer rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TechKind {
    Attack,
    Armor,
}
/// One purchaseable technology identity per age and kind; no research executes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TechSpec {
    pub id: Id<'static>,
    pub design_version: u16,
    pub age: Age,
    pub kind: TechKind,
    pub cost_gold: u16,
    pub research: Ticks,
    pub bonus_bp: BasisPoints,
}
impl TechSpec {
    /// Check descriptor-local bounds; duplicate research is rejected by future C01.
    pub fn validate(&self) -> Result<Validated<'_, Self>, SchemaError> {
        Id::new(self.id.as_str())?;
        if self.design_version != 1 {
            return Err(SchemaError::UnsupportedDesignVersion);
        }
        if self.cost_gold == 0
            || self.cost_gold > 1_000
            || self.research.get() != 60
            || self.bonus_bp.get()
                != match self.kind {
                    TechKind::Attack => 1_000,
                    TechKind::Armor => 250,
                }
        {
            return Err(SchemaError::InvalidPayload);
        }
        Ok(Validated { spec: self })
    }
}

/// Proposed advancement data; total XP is a non-spending eligibility threshold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AgeAdvanceSpec {
    pub to_age: Age,
    pub gold: u16,
    pub total_xp: u16,
    pub duration: Ticks,
}
