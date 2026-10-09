//! Honest unavailable future seams; no fake success path (doc 00 §7, I-16).

use std::{convert::Infallible, fmt};

/// Future surface requested by a caller; all remain outside D01.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeferredSurface {
    /// Pure logical-tick simulation requires a separately authorized C01.
    Simulation,
    /// Projection-only AI requires an implemented and validated simulator.
    AiPlanning,
    /// Snapshot compatibility requires an actual versioned canonical schema.
    SnapshotMigration,
}

/// A typed D01 refusal naming the future surface and its ownership gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhaseGateError {
    /// Which future surface is unavailable.
    pub surface: DeferredSurface,
}

impl fmt::Display for PhaseGateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:?} unavailable: PHASE D01; C01 requires D06 owner approval",
            self.surface
        )
    }
}

impl std::error::Error for PhaseGateError {}

/// Request simulation; D01 cannot construct a successful simulation result.
pub const fn request_simulation() -> Result<Infallible, PhaseGateError> {
    Err(PhaseGateError {
        surface: DeferredSurface::Simulation,
    })
}

/// Request an AI plan; there is no gameplay or bot implementation in D01.
pub const fn request_ai_plan() -> Result<Infallible, PhaseGateError> {
    Err(PhaseGateError {
        surface: DeferredSurface::AiPlanning,
    })
}

/// Request migration; all versions and payloads are explicitly unsupported.
/// Bytes are not decoded and cannot be confused with a supported replay.
pub const fn request_snapshot_migration(
    _from_design_version: u16,
    _bytes: &[u8],
) -> Result<Infallible, PhaseGateError> {
    Err(PhaseGateError {
        surface: DeferredSurface::SnapshotMigration,
    })
}
