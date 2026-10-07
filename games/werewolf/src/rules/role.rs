#![allow(clippy::doc_markdown)] // `@ai.*` values are machine-readable paths.

//! Werewolf roles, alignments, and preset distribution tables. (doc 02 §12.3, doc 08 §5.1)
//!
//! @ai.role domain-types
//! @ai.domain werewolf.rules.role
//! @ai.pure true
//! @ai.invariant base-roles-closed
//! @ai.invariant alignment-mapping-total
//! @ai.invariant classic-v1-counts-sum-to-seats
//! @ai.evidence tests::config::role_all_contains_all_six_roles_uniquely
//! @ai.evidence tests::config::role_alignment_mapping_is_correct
//! @ai.evidence tests::config::classic_v1_table_matches_pinned_specification_for_all_seat_counts

use serde::{Deserialize, Serialize};

use super::config::{SeatCount, SeatCountError, MAX_SEATS, MIN_SEATS};

/// The six base Werewolf roles supported in Phase 3. (doc 08 §5.1)
///
/// Advanced roles (Cupid/lovers, Jester, Alpha) are Phase 9+ and explicitly
/// out of Phase-3 scope.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub enum Role {
    Villager,
    Werewolf,
    Seer,
    Doctor,
    Hunter,
    Witch,
}

impl Role {
    /// Array containing all six base roles in canonical order.
    pub const ALL: [Self; 6] = [
        Self::Villager,
        Self::Werewolf,
        Self::Seer,
        Self::Doctor,
        Self::Hunter,
        Self::Witch,
    ];

    /// Returns the team alignment for this role.
    #[must_use]
    pub const fn alignment(self) -> Alignment {
        match self {
            Self::Villager | Self::Seer | Self::Doctor | Self::Hunter | Self::Witch => {
                Alignment::Village
            }
            Self::Werewolf => Alignment::Wolf,
        }
    }

    /// Returns `true` if this role aligns with the Werewolf team.
    #[must_use]
    pub const fn is_wolf(self) -> bool {
        matches!(self.alignment(), Alignment::Wolf)
    }

    /// Returns `true` if this role aligns with the Village team.
    #[must_use]
    pub const fn is_village(self) -> bool {
        matches!(self.alignment(), Alignment::Village)
    }

    /// Human-readable role name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Villager => "Villager",
            Self::Werewolf => "Werewolf",
            Self::Seer => "Seer",
            Self::Doctor => "Doctor",
            Self::Hunter => "Hunter",
            Self::Witch => "Witch",
        }
    }
}

/// Team faction alignment in Werewolf.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub enum Alignment {
    /// The Village team: seeks to eliminate all werewolves.
    Village,
    /// The Wolf team: seeks to equal or outnumber living non-werewolves.
    Wolf,
}

/// Exact breakdown of roles assigned to a match.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct RoleCounts {
    pub werewolves: u8,
    pub seers: u8,
    pub doctors: u8,
    pub hunters: u8,
    pub witches: u8,
    pub villagers: u8,
}

impl RoleCounts {
    /// Total number of roles in this breakdown.
    #[must_use]
    pub const fn total(self) -> u8 {
        self.werewolves
            .saturating_add(self.seers)
            .saturating_add(self.doctors)
            .saturating_add(self.hunters)
            .saturating_add(self.witches)
            .saturating_add(self.villagers)
    }

    /// Returns the count assigned for a specific role.
    #[must_use]
    pub const fn count(self, role: Role) -> u8 {
        match role {
            Role::Werewolf => self.werewolves,
            Role::Seer => self.seers,
            Role::Doctor => self.doctors,
            Role::Hunter => self.hunters,
            Role::Witch => self.witches,
            Role::Villager => self.villagers,
        }
    }

    /// Produces the full multiset of roles in deterministic canonical order.
    #[must_use]
    pub fn multiset(self) -> Vec<Role> {
        let mut roles = Vec::with_capacity(self.total() as usize);
        for _ in 0..self.werewolves {
            roles.push(Role::Werewolf);
        }
        for _ in 0..self.seers {
            roles.push(Role::Seer);
        }
        for _ in 0..self.doctors {
            roles.push(Role::Doctor);
        }
        for _ in 0..self.hunters {
            roles.push(Role::Hunter);
        }
        for _ in 0..self.witches {
            roles.push(Role::Witch);
        }
        for _ in 0..self.villagers {
            roles.push(Role::Villager);
        }
        roles
    }
}

/// Named role-distribution presets. (W-D1)
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Default, Serialize, Deserialize)]
pub enum Preset {
    /// The canonical `ClassicV1` preset family, indexing a fixed role distribution for 6..=20 seats.
    #[default]
    ClassicV1,
}

impl Preset {
    /// Returns the exact role counts for this preset and validated seat count.
    #[must_use]
    pub const fn role_counts(self, seats: SeatCount) -> RoleCounts {
        match self {
            Self::ClassicV1 => classic_v1_role_counts(seats),
        }
    }

    /// Convenience validator mapping an arbitrary `u8` seat count to its role counts if valid.
    ///
    /// # Errors
    /// Returns [`SeatCountError`] if `seats` is not within [`MIN_SEATS`..=[`MAX_SEATS`]].
    pub fn counts_for_seat_count(self, seats: u8) -> Result<RoleCounts, SeatCountError> {
        SeatCount::new(seats).map(|sc| self.role_counts(sc))
    }
}

/// The authoritative `ClassicV1` role count table for 6..=20 seats (W-D1).
///
/// Table columns: `n` seats, `W` werewolves, `S` seers, `D` doctors, `H` hunters, `T` witches, `V` villagers.
/// `V = n - W - S - D - H - T`.
const CLASSIC_V1_TABLE: [RoleCounts; (MAX_SEATS - MIN_SEATS + 1) as usize] = [
    // 6 seats: 1W, 1S, 1D, 0H, 0T, 3V
    RoleCounts {
        werewolves: 1,
        seers: 1,
        doctors: 1,
        hunters: 0,
        witches: 0,
        villagers: 3,
    },
    // 7 seats: 1W, 1S, 1D, 0H, 0T, 4V
    RoleCounts {
        werewolves: 1,
        seers: 1,
        doctors: 1,
        hunters: 0,
        witches: 0,
        villagers: 4,
    },
    // 8 seats: 2W, 1S, 1D, 1H, 0T, 3V
    RoleCounts {
        werewolves: 2,
        seers: 1,
        doctors: 1,
        hunters: 1,
        witches: 0,
        villagers: 3,
    },
    // 9 seats: 2W, 1S, 1D, 1H, 0T, 4V
    RoleCounts {
        werewolves: 2,
        seers: 1,
        doctors: 1,
        hunters: 1,
        witches: 0,
        villagers: 4,
    },
    // 10 seats: 2W, 1S, 1D, 1H, 1T, 4V
    RoleCounts {
        werewolves: 2,
        seers: 1,
        doctors: 1,
        hunters: 1,
        witches: 1,
        villagers: 4,
    },
    // 11 seats: 2W, 1S, 1D, 1H, 1T, 5V
    RoleCounts {
        werewolves: 2,
        seers: 1,
        doctors: 1,
        hunters: 1,
        witches: 1,
        villagers: 5,
    },
    // 12 seats: 3W, 1S, 1D, 1H, 1T, 5V
    RoleCounts {
        werewolves: 3,
        seers: 1,
        doctors: 1,
        hunters: 1,
        witches: 1,
        villagers: 5,
    },
    // 13 seats: 3W, 1S, 1D, 1H, 1T, 6V
    RoleCounts {
        werewolves: 3,
        seers: 1,
        doctors: 1,
        hunters: 1,
        witches: 1,
        villagers: 6,
    },
    // 14 seats: 3W, 1S, 1D, 1H, 1T, 7V
    RoleCounts {
        werewolves: 3,
        seers: 1,
        doctors: 1,
        hunters: 1,
        witches: 1,
        villagers: 7,
    },
    // 15 seats: 3W, 1S, 1D, 1H, 1T, 8V
    RoleCounts {
        werewolves: 3,
        seers: 1,
        doctors: 1,
        hunters: 1,
        witches: 1,
        villagers: 8,
    },
    // 16 seats: 4W, 1S, 1D, 1H, 1T, 8V
    RoleCounts {
        werewolves: 4,
        seers: 1,
        doctors: 1,
        hunters: 1,
        witches: 1,
        villagers: 8,
    },
    // 17 seats: 4W, 1S, 1D, 1H, 1T, 9V
    RoleCounts {
        werewolves: 4,
        seers: 1,
        doctors: 1,
        hunters: 1,
        witches: 1,
        villagers: 9,
    },
    // 18 seats: 4W, 1S, 1D, 1H, 1T, 10V
    RoleCounts {
        werewolves: 4,
        seers: 1,
        doctors: 1,
        hunters: 1,
        witches: 1,
        villagers: 10,
    },
    // 19 seats: 4W, 1S, 1D, 1H, 1T, 11V
    RoleCounts {
        werewolves: 4,
        seers: 1,
        doctors: 1,
        hunters: 1,
        witches: 1,
        villagers: 11,
    },
    // 20 seats: 5W, 1S, 1D, 1H, 1T, 11V
    RoleCounts {
        werewolves: 5,
        seers: 1,
        doctors: 1,
        hunters: 1,
        witches: 1,
        villagers: 11,
    },
];

/// Returns the exact `ClassicV1` role count for the given validated seat count.
#[must_use]
pub const fn classic_v1_role_counts(seats: SeatCount) -> RoleCounts {
    let index = (seats.get() - MIN_SEATS) as usize;
    CLASSIC_V1_TABLE[index]
}
