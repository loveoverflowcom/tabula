//! Source-only embedded-surface backend draft (ADR-0043).
//!
//! The pinned Macroquad/Miniquad backend has no supported CMP-owned context bootstrap,
//! synchronous surface fence or retained worker join. Every operation below returns a named
//! unavailable error. It neither calls the standalone bootstrap nor owns a worker, context,
//! match, lifecycle coordinator or callback. PR #108 remains the mobile lifecycle owner.
//!
//! `EmbeddedSurfaceProposalV1` is a proposed field contract, not a connected or stable ABI:
//! there is no C layout, serialization, exported symbol, JNI mapping or pointer conversion.
//! A surface token would need an approved platform-owned reference table. Validating its
//! integer does not acquire a native window or establish that such a reference exists.

use std::fmt;

/// First proposed field-contract version; no ABI has been approved (ADR-0043).
pub const EMBEDDED_SURFACE_PROPOSAL_VERSION: u32 = 1;
const MAX_HOST_TOKEN: u64 = i64::MAX as u64;

/// Process-local PR #108 worker correlation; not a worker ownership witness.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuntimeId(u64);

impl RuntimeId {
    /// Accepts only positive values representable by the Kotlin owner's `Long`.
    pub fn new(value: u64) -> Result<Self, EmbeddedBoundaryError> {
        if value == 0 || value > MAX_HOST_TOKEN {
            return Err(EmbeddedBoundaryError::RuntimeIdOutOfRange);
        }
        Ok(Self(value))
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Correlation for one worker and its dispatched surface revision (PR #108).
///
/// A lease alone proves neither attachment nor currentness. The existing host coordinator
/// owns generation admission; a future backend must also fence stale worker callbacks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SurfaceLease {
    runtime: RuntimeId,
    revision: u64,
}

impl SurfaceLease {
    pub fn new(runtime: RuntimeId, revision: u64) -> Result<Self, EmbeddedBoundaryError> {
        if revision == 0 || revision > MAX_HOST_TOKEN {
            return Err(EmbeddedBoundaryError::SurfaceRevisionOutOfRange);
        }
        Ok(Self { runtime, revision })
    }

    pub const fn runtime(self) -> RuntimeId {
        self.runtime
    }

    pub const fn revision(self) -> u64 {
        self.revision
    }
}

/// Proposed opaque platform reference-table key, never an address (ADR-0043).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SurfaceToken(u64);

impl SurfaceToken {
    pub fn new(value: u64) -> Result<Self, EmbeddedBoundaryError> {
        if value == 0 || value > MAX_HOST_TOKEN {
            return Err(EmbeddedBoundaryError::SurfaceTokenOutOfRange);
        }
        Ok(Self(value))
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Validated game-view physical pixels and density, matching PR #108's host bounds.
///
/// The outer CMP shell already consumes insets. This draft performs no conversion; the future
/// renderer/input owner must apply density once without changing rules or match lifetime.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceGeometry {
    width_px: u32,
    height_px: u32,
    density: f32,
}

impl SurfaceGeometry {
    pub fn new(width_px: u32, height_px: u32, density: f32) -> Result<Self, EmbeddedBoundaryError> {
        if !(1..=16_384).contains(&width_px) {
            return Err(EmbeddedBoundaryError::WidthOutOfRange);
        }
        if !(1..=16_384).contains(&height_px) {
            return Err(EmbeddedBoundaryError::HeightOutOfRange);
        }
        if !density.is_finite() || !(0.5..=16.0).contains(&density) {
            return Err(EmbeddedBoundaryError::DensityOutOfRange);
        }
        Ok(Self {
            width_px,
            height_px,
            density,
        })
    }

    pub const fn width_px(self) -> u32 {
        self.width_px
    }

    pub const fn height_px(self) -> u32 {
        self.height_px
    }

    pub const fn density(self) -> f32 {
        self.density
    }
}

/// Owned scalar fields proposed for a future reviewed platform boundary (ADR-0043).
///
/// Public fields deliberately admit invalid data; convert before consuming. This is not
/// `repr(C)` and is not exported, serialized, an Android `Surface` or a native pointer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EmbeddedSurfaceProposalV1 {
    pub version: u32,
    pub runtime_id: u64,
    pub surface_revision: u64,
    pub surface_token: u64,
    pub width_px: u32,
    pub height_px: u32,
    pub density: f32,
}

/// Validated field values only; construction does not initialize a graphics context.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EmbeddedAttachRequest {
    pub lease: SurfaceLease,
    pub surface: SurfaceToken,
    pub geometry: SurfaceGeometry,
}

impl TryFrom<EmbeddedSurfaceProposalV1> for EmbeddedAttachRequest {
    type Error = EmbeddedBoundaryError;

    fn try_from(value: EmbeddedSurfaceProposalV1) -> Result<Self, Self::Error> {
        if value.version != EMBEDDED_SURFACE_PROPOSAL_VERSION {
            return Err(EmbeddedBoundaryError::UnsupportedProposalVersion);
        }
        let runtime = RuntimeId::new(value.runtime_id)?;
        Ok(Self {
            lease: SurfaceLease::new(runtime, value.surface_revision)?,
            surface: SurfaceToken::new(value.surface_token)?,
            geometry: SurfaceGeometry::new(value.width_px, value.height_px, value.density)?,
        })
    }
}

/// Bounded invalid-field diagnostics; no private state or unbounded text (I-5/I-10).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmbeddedBoundaryError {
    UnsupportedProposalVersion,
    RuntimeIdOutOfRange,
    SurfaceRevisionOutOfRange,
    SurfaceTokenOutOfRange,
    WidthOutOfRange,
    HeightOutOfRange,
    DensityOutOfRange,
}

impl fmt::Display for EmbeddedBoundaryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid embedded-surface field: {self:?}")
    }
}

impl std::error::Error for EmbeddedBoundaryError {}

/// Requested backend effect, separate from the existing host lifecycle coordinator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmbeddedOperation {
    AttachContext,
    Resize,
    FenceSurface,
    Rendering,
    Detach,
    StopAndJoin,
}

/// Concrete mechanisms missing from the pinned supported backend (ADR-0043).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MissingEmbeddedMechanism {
    ExternalSurfaceContextBootstrap,
    OwnerThreadContextResize,
    SynchronousSurfaceAccessBarrier,
    SuspendedFrameScheduling,
    OwnerThreadSurfaceRelease,
    RetainedWorkerJoinAndContextTeardown,
}

/// Explicit backend unavailability; an error never establishes readiness, fencing or join.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmbeddedSurfaceError {
    Invalid(EmbeddedBoundaryError),
    Unavailable {
        operation: EmbeddedOperation,
        missing: MissingEmbeddedMechanism,
    },
}

impl fmt::Display for EmbeddedSurfaceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(error) => error.fmt(formatter),
            Self::Unavailable { operation, missing } => {
                write!(
                    formatter,
                    "embedded backend unavailable for {operation:?}: {missing:?}"
                )
            }
        }
    }
}

impl std::error::Error for EmbeddedSurfaceError {}

/// Stateless source skeleton. Instantiation starts nothing and grants no runtime capability.
#[derive(Clone, Copy, Debug, Default)]
pub struct EmbeddedSurfaceBackendDraft;

impl EmbeddedSurfaceBackendDraft {
    pub fn attach_context(
        &self,
        proposal: EmbeddedSurfaceProposalV1,
    ) -> Result<(), EmbeddedSurfaceError> {
        let _request =
            EmbeddedAttachRequest::try_from(proposal).map_err(EmbeddedSurfaceError::Invalid)?;
        // TODO(phase 6): reviewed existing-Surface/context bootstrap and platform reference
        // acquisition on the GPU owner thread, without standalone miniquad::start.
        Err(unavailable(
            EmbeddedOperation::AttachContext,
            MissingEmbeddedMechanism::ExternalSurfaceContextBootstrap,
        ))
    }

    pub fn resize(
        &self,
        _lease: SurfaceLease,
        _geometry: SurfaceGeometry,
    ) -> Result<(), EmbeddedSurfaceError> {
        // TODO(phase 6): resize the existing owned context; surface recreation must not create a match.
        Err(unavailable(
            EmbeddedOperation::Resize,
            MissingEmbeddedMechanism::OwnerThreadContextResize,
        ))
    }

    /// Must eventually return success only after the exact surface cannot be touched again.
    /// This draft returns an error even for repeated requests; it never manufactures a fence.
    pub fn fence_surface(&self, _lease: SurfaceLease) -> Result<(), EmbeddedSurfaceError> {
        // TODO(phase 6): synchronous access/input barrier safe during reentrant surfaceDestroyed;
        // worker callbacks must not synchronously wait for the fencing UI thread.
        Err(unavailable(
            EmbeddedOperation::FenceSurface,
            MissingEmbeddedMechanism::SynchronousSurfaceAccessBarrier,
        ))
    }

    pub fn rendering(
        &self,
        _lease: SurfaceLease,
        _enabled: bool,
    ) -> Result<(), EmbeddedSurfaceError> {
        // TODO(phase 6): suspend actual frame scheduling/GPU work, without changing rule clocks.
        Err(unavailable(
            EmbeddedOperation::Rendering,
            MissingEmbeddedMechanism::SuspendedFrameScheduling,
        ))
    }

    pub fn detach(&self, _lease: SurfaceLease) -> Result<(), EmbeddedSurfaceError> {
        // TODO(phase 6): release context-bound surface/window references after a real fence.
        Err(unavailable(
            EmbeddedOperation::Detach,
            MissingEmbeddedMechanism::OwnerThreadSurfaceRelease,
        ))
    }

    /// Future-shaped stop contract only; polling the error is not a worker completion.
    pub fn stop_and_join(
        &self,
        _runtime: RuntimeId,
    ) -> impl std::future::Future<Output = Result<(), EmbeddedSurfaceError>> {
        // TODO(phase 6): retain exact worker handle, cancel pre-surface initialization, stop
        // frames, destroy Macroquad context on its owner thread, release GPU/window/JNI refs,
        // and complete the real join off the UI thread before acknowledging Stopped/reopen.
        std::future::ready(Err(unavailable(
            EmbeddedOperation::StopAndJoin,
            MissingEmbeddedMechanism::RetainedWorkerJoinAndContextTeardown,
        )))
    }
}

const fn unavailable(
    operation: EmbeddedOperation,
    missing: MissingEmbeddedMechanism,
) -> EmbeddedSurfaceError {
    EmbeddedSurfaceError::Unavailable { operation, missing }
}

#[cfg(test)]
mod tests {
    use std::{
        future::Future,
        pin::pin,
        task::{Context, Poll, Waker},
    };

    use super::*;

    fn proposal() -> EmbeddedSurfaceProposalV1 {
        EmbeddedSurfaceProposalV1 {
            version: 1,
            runtime_id: 1,
            surface_revision: 1,
            surface_token: 1,
            width_px: 320,
            height_px: 240,
            density: 2.0,
        }
    }

    fn lease() -> SurfaceLease {
        SurfaceLease::new(RuntimeId::new(1).unwrap(), 1).unwrap()
    }

    #[test]
    fn tokens_reject_zero_and_signed_host_overflow() {
        for value in [0, MAX_HOST_TOKEN + 1, u64::MAX] {
            assert_eq!(
                RuntimeId::new(value),
                Err(EmbeddedBoundaryError::RuntimeIdOutOfRange)
            );
            assert_eq!(
                SurfaceToken::new(value),
                Err(EmbeddedBoundaryError::SurfaceTokenOutOfRange)
            );
            assert_eq!(
                SurfaceLease::new(RuntimeId::new(1).unwrap(), value),
                Err(EmbeddedBoundaryError::SurfaceRevisionOutOfRange)
            );
        }
        for value in [1, MAX_HOST_TOKEN] {
            assert_eq!(RuntimeId::new(value).unwrap().get(), value);
            assert_eq!(SurfaceToken::new(value).unwrap().get(), value);
            assert_eq!(
                SurfaceLease::new(RuntimeId::new(value).unwrap(), value)
                    .unwrap()
                    .revision(),
                value
            );
        }
    }

    #[test]
    fn geometry_checks_each_invalid_bound_and_nonfinite_density() {
        for value in [0, 16_385, u32::MAX] {
            assert_eq!(
                SurfaceGeometry::new(value, 1, 1.0),
                Err(EmbeddedBoundaryError::WidthOutOfRange)
            );
            assert_eq!(
                SurfaceGeometry::new(1, value, 1.0),
                Err(EmbeddedBoundaryError::HeightOutOfRange)
            );
        }
        for density in [0.0, 0.49, 16.01, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(
                SurfaceGeometry::new(1, 1, density),
                Err(EmbeddedBoundaryError::DensityOutOfRange)
            );
        }
        for (width, height, density) in [(1, 1, 0.5), (16_384, 16_384, 16.0)] {
            let geometry = SurfaceGeometry::new(width, height, density).unwrap();
            assert_eq!((geometry.width_px(), geometry.height_px()), (width, height));
            assert_eq!(geometry.density().to_bits(), density.to_bits());
        }
    }

    #[test]
    fn proposal_validation_does_not_bypass_refined_fields() {
        let base = proposal();
        for (value, error) in [
            (
                EmbeddedSurfaceProposalV1 { version: 0, ..base },
                EmbeddedBoundaryError::UnsupportedProposalVersion,
            ),
            (
                EmbeddedSurfaceProposalV1 { version: 2, ..base },
                EmbeddedBoundaryError::UnsupportedProposalVersion,
            ),
            (
                EmbeddedSurfaceProposalV1 {
                    runtime_id: 0,
                    ..base
                },
                EmbeddedBoundaryError::RuntimeIdOutOfRange,
            ),
            (
                EmbeddedSurfaceProposalV1 {
                    surface_revision: 0,
                    ..base
                },
                EmbeddedBoundaryError::SurfaceRevisionOutOfRange,
            ),
            (
                EmbeddedSurfaceProposalV1 {
                    surface_token: 0,
                    ..base
                },
                EmbeddedBoundaryError::SurfaceTokenOutOfRange,
            ),
            (
                EmbeddedSurfaceProposalV1 {
                    width_px: 0,
                    ..base
                },
                EmbeddedBoundaryError::WidthOutOfRange,
            ),
            (
                EmbeddedSurfaceProposalV1 {
                    height_px: 0,
                    ..base
                },
                EmbeddedBoundaryError::HeightOutOfRange,
            ),
            (
                EmbeddedSurfaceProposalV1 {
                    density: f32::NAN,
                    ..base
                },
                EmbeddedBoundaryError::DensityOutOfRange,
            ),
        ] {
            assert_eq!(EmbeddedAttachRequest::try_from(value), Err(error));
            assert_eq!(
                EmbeddedSurfaceBackendDraft.attach_context(value),
                Err(EmbeddedSurfaceError::Invalid(error))
            );
        }
        let request = EmbeddedAttachRequest::try_from(base).unwrap();
        assert_eq!(request.lease, lease());
        assert_eq!(request.surface.get(), 1);
    }

    #[test]
    fn valid_attach_remains_unavailable_without_initializing_context() {
        assert_eq!(
            EmbeddedSurfaceBackendDraft.attach_context(proposal()),
            Err(unavailable(
                EmbeddedOperation::AttachContext,
                MissingEmbeddedMechanism::ExternalSurfaceContextBootstrap
            ))
        );
    }

    #[test]
    fn resize_remains_unavailable_without_restarting_match() {
        assert_eq!(
            EmbeddedSurfaceBackendDraft.resize(lease(), SurfaceGeometry::new(1, 1, 0.5).unwrap()),
            Err(unavailable(
                EmbeddedOperation::Resize,
                MissingEmbeddedMechanism::OwnerThreadContextResize
            ))
        );
    }

    #[test]
    fn repeated_surface_fence_never_reports_success() {
        for _ in 0..2 {
            assert_eq!(
                EmbeddedSurfaceBackendDraft.fence_surface(lease()),
                Err(unavailable(
                    EmbeddedOperation::FenceSurface,
                    MissingEmbeddedMechanism::SynchronousSurfaceAccessBarrier
                ))
            );
        }
    }

    #[test]
    fn rendering_enable_and_disable_both_remain_unavailable() {
        for enabled in [false, true] {
            assert_eq!(
                EmbeddedSurfaceBackendDraft.rendering(lease(), enabled),
                Err(unavailable(
                    EmbeddedOperation::Rendering,
                    MissingEmbeddedMechanism::SuspendedFrameScheduling
                ))
            );
        }
    }

    #[test]
    fn detach_does_not_claim_references_released() {
        assert_eq!(
            EmbeddedSurfaceBackendDraft.detach(lease()),
            Err(unavailable(
                EmbeddedOperation::Detach,
                MissingEmbeddedMechanism::OwnerThreadSurfaceRelease
            ))
        );
    }

    #[test]
    fn async_stop_cannot_fabricate_worker_join() {
        let backend = EmbeddedSurfaceBackendDraft;
        let mut future = pin!(backend.stop_and_join(lease().runtime()));
        let waker = Waker::noop();
        assert_eq!(
            future.as_mut().poll(&mut Context::from_waker(waker)),
            Poll::Ready(Err(unavailable(
                EmbeddedOperation::StopAndJoin,
                MissingEmbeddedMechanism::RetainedWorkerJoinAndContextTeardown
            )))
        );
    }

    #[test]
    fn failed_cleanup_cannot_change_backend_availability() {
        let backend = EmbeddedSurfaceBackendDraft;
        let initial = backend.attach_context(proposal());
        assert!(backend.fence_surface(lease()).is_err());
        assert!(backend.detach(lease()).is_err());
        assert_eq!(backend.attach_context(proposal()), initial);
        assert_eq!(
            backend.attach_context(EmbeddedSurfaceProposalV1 {
                runtime_id: 2,
                surface_revision: 2,
                ..proposal()
            }),
            initial
        );
    }
}
