//! Source-only native `GameHost` composition draft (ADR-0043, I-9/I-10).
//!
//! This is not a second lifecycle engine or a playable runtime. PR #108's Kotlin owner/session
//! remain the sole mobile lifecycle coordinator. The inventory is empty even when standalone
//! game features compile. No registry-native composition, selected local match/presenter,
//! verified native assets, worker, callback or platform ABI is implemented here.
//!
//! Future selection must use `tabula-registry`'s erased interfaces, then the existing
//! `LocalMatch`/projection/presenter/resource pipeline. An opaque `GameId` or a public discovery
//! entry does not establish packaged runtime availability. No canonical state or per-frame
//! `RenderList` is carried in these host controls (I-5/I-6/I-10).

use std::fmt;

use tabula_core::GameId;
use tabula_render_macroquad::embedded::{
    EmbeddedSurfaceBackendDraft, EmbeddedSurfaceError, EmbeddedSurfaceProposalV1, RuntimeId,
    SurfaceGeometry, SurfaceLease,
};

/// Explicit shell-resolved theme. System resolution belongs to CMP (ADR-0043).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeTheme {
    Light,
    Dark,
    HighContrastLight,
    HighContrastDark,
}

/// Existing bounded shell locale choices; no device-settings polling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeLocale {
    Vietnamese,
    English,
}

/// Launch-only host facts. Requested keep-awake is not a permission or service grant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeLaunchPreferences {
    pub theme: NativeTheme,
    pub reduced_motion: bool,
    pub locale: NativeLocale,
    pub request_keep_awake: bool,
}

/// Bounded opaque selection and explicit preferences, not native launch admission.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeLaunchRequest {
    game_id: GameId,
    preferences: NativeLaunchPreferences,
}

impl NativeLaunchRequest {
    pub fn new(
        game_id: GameId,
        preferences: NativeLaunchPreferences,
    ) -> Result<Self, NativeBoundaryError> {
        validate_game_id(&game_id)?;
        Ok(Self {
            game_id,
            preferences,
        })
    }

    pub fn game_id(&self) -> &GameId {
        &self.game_id
    }

    pub const fn preferences(&self) -> NativeLaunchPreferences {
        self.preferences
    }
}

/// Platform pointer facts only; they do not encode a game move (I-10).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativePointerPhase {
    Down,
    Move,
    Up,
}

/// Validated pointer in local game-view physical pixels, matching PR #108's bounds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativePointer {
    id: i32,
    phase: NativePointerPhase,
    x_px: f32,
    y_px: f32,
}

impl NativePointer {
    pub fn new(
        id: i32,
        phase: NativePointerPhase,
        x_px: f32,
        y_px: f32,
    ) -> Result<Self, NativeBoundaryError> {
        if !(0..=31).contains(&id) {
            return Err(NativeBoundaryError::PointerIdOutOfRange);
        }
        if !x_px.is_finite() || !(-32_768.0..=32_768.0).contains(&x_px) {
            return Err(NativeBoundaryError::PointerXOutOfRange);
        }
        if !y_px.is_finite() || !(-32_768.0..=32_768.0).contains(&y_px) {
            return Err(NativeBoundaryError::PointerYOutOfRange);
        }
        Ok(Self {
            id,
            phase,
            x_px,
            y_px,
        })
    }

    pub const fn id(self) -> i32 {
        self.id
    }

    pub const fn phase(self) -> NativePointerPhase {
        self.phase
    }

    pub const fn position_px(self) -> (f32, f32) {
        (self.x_px, self.y_px)
    }
}

/// Existing host-service reply facts; no platform service implementation is supplied.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeServiceReplyCode {
    Ok,
    Denied,
    Unsupported,
}

/// Proposed safe Rust control values, not a serialized or connected platform ABI.
#[derive(Clone, Debug, PartialEq)]
pub enum NativeHostCommand {
    Start {
        runtime: RuntimeId,
        request: NativeLaunchRequest,
        foreground: bool,
    },
    Attach(EmbeddedSurfaceProposalV1),
    Resize {
        lease: SurfaceLease,
        geometry: SurfaceGeometry,
    },
    Detach {
        lease: SurfaceLease,
    },
    Rendering {
        lease: SurfaceLease,
        enabled: bool,
    },
    Pointer {
        lease: SurfaceLease,
        input: NativePointer,
    },
    CancelPointers {
        lease: SurfaceLease,
    },
    Foreground {
        runtime: RuntimeId,
        active: bool,
    },
    Back {
        runtime: RuntimeId,
    },
    ServiceReply {
        runtime: RuntimeId,
        request_id: i32,
        code: NativeServiceReplyCode,
    },
    Stop {
        runtime: RuntimeId,
    },
}

/// Bounded invalid input diagnostics, independent of game rules or private state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeBoundaryError {
    GameIdTooLong,
    PointerIdOutOfRange,
    PointerXOutOfRange,
    PointerYOutOfRange,
    ServiceRequestIdOutOfRange,
}

impl fmt::Display for NativeBoundaryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid native-host field: {self:?}")
    }
}

impl std::error::Error for NativeBoundaryError {}

fn validate_game_id(game_id: &GameId) -> Result<(), NativeBoundaryError> {
    if game_id.as_str().len() > 80 {
        return Err(NativeBoundaryError::GameIdTooLong);
    }
    Ok(())
}

/// Missing registry/first-party composition mechanisms; no native registry API is implied.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MissingNativeComposition {
    PackagedRegistrySelectedLocalRuntimeAndVerifiedAssets,
    SelectedVerifiedAssetPackAndContextUpload,
}

/// Non-renderer controls whose actual runtime owner has not been composed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeControlOperation {
    Pointer,
    CancelPointers,
    Foreground,
    Back,
    ServiceReply,
}

/// Concrete runtime mechanisms not implemented by this source skeleton.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MissingNativeControl {
    OwnerThreadInputAdmissionAndDensityConversion,
    LocalPointerCancellation,
    ForegroundRuntimeAdmission,
    RuntimeLeaveConfirmation,
    ScopedHostServiceReplyRouting,
}

/// Typed fail-closed result; no variant is a readiness/fence/termination acknowledgement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeHostError {
    Invalid(NativeBoundaryError),
    Unavailable {
        game_id: GameId,
        missing: MissingNativeComposition,
    },
    NotImplemented {
        operation: NativeControlOperation,
        missing: MissingNativeControl,
    },
    Backend(EmbeddedSurfaceError),
}

impl fmt::Display for NativeHostError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(error) => write!(formatter, "invalid native-host field: {error:?}"),
            Self::Unavailable { game_id, missing } => write!(
                formatter,
                "native runtime unavailable for {game_id}: {missing:?}"
            ),
            Self::NotImplemented { operation, missing } => write!(
                formatter,
                "native control not implemented for {operation:?}: {missing:?}"
            ),
            Self::Backend(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for NativeHostError {}

/// Empty shipping inventory. Standalone compiled game features grant no native support.
#[derive(Clone, Copy, Debug, Default)]
pub struct NativeRuntimeInventoryDraft;

impl NativeRuntimeInventoryDraft {
    pub const fn packaged_games(&self) -> &'static [GameId] {
        &[]
    }

    /// TODO(phase 6): registry-erased selection of an actually packaged first-party local
    /// runtime, projection/presenter and verified assets; no game-id dispatch in this shell.
    pub fn resolve_selected_runtime(&self, game_id: &GameId) -> Result<(), NativeHostError> {
        validate_game_id(game_id).map_err(NativeHostError::Invalid)?;
        Err(NativeHostError::Unavailable {
            game_id: game_id.clone(),
            missing:
                MissingNativeComposition::PackagedRegistrySelectedLocalRuntimeAndVerifiedAssets,
        })
    }

    /// TODO(phase 6): compose the existing selected asset identity/integrity/loading pipeline;
    /// GPU uploads require the real current context/thread. No resources become ready here.
    pub fn prepare_selected_assets(&self, game_id: &GameId) -> Result<(), NativeHostError> {
        validate_game_id(game_id).map_err(NativeHostError::Invalid)?;
        Err(NativeHostError::Unavailable {
            game_id: game_id.clone(),
            missing: MissingNativeComposition::SelectedVerifiedAssetPackAndContextUpload,
        })
    }
}

/// Stateless contract composition only, with no lifecycle, worker or event emitter.
#[derive(Debug, Default)]
pub struct NativeGameHostDraft {
    backend: EmbeddedSurfaceBackendDraft,
    inventory: NativeRuntimeInventoryDraft,
}

impl NativeGameHostDraft {
    pub const fn inventory(&self) -> &NativeRuntimeInventoryDraft {
        &self.inventory
    }

    /// Control future only. It starts/schedules nothing and never emits runtime events.
    pub async fn execute(&self, command: NativeHostCommand) -> Result<(), NativeHostError> {
        match command {
            NativeHostCommand::Start {
                runtime: _,
                request,
                foreground: _,
            } => self.inventory.resolve_selected_runtime(request.game_id()),
            NativeHostCommand::Attach(proposal) => self
                .backend
                .attach_context(proposal)
                .map_err(NativeHostError::Backend),
            NativeHostCommand::Resize { lease, geometry } => self
                .backend
                .resize(lease, geometry)
                .map_err(NativeHostError::Backend),
            NativeHostCommand::Detach { lease } => {
                self.backend.detach(lease).map_err(NativeHostError::Backend)
            }
            NativeHostCommand::Rendering { lease, enabled } => self
                .backend
                .rendering(lease, enabled)
                .map_err(NativeHostError::Backend),
            NativeHostCommand::Stop { runtime } => self
                .backend
                .stop_and_join(runtime)
                .await
                .map_err(NativeHostError::Backend),
            NativeHostCommand::Pointer { lease: _, input: _ } => {
                // TODO(phase 6): owner-thread admission/correlation, one density conversion,
                // then existing presenter InputEvent/intent dispatch, never Kotlin game rules.
                Err(not_implemented(
                    NativeControlOperation::Pointer,
                    MissingNativeControl::OwnerThreadInputAdmissionAndDensityConversion,
                ))
            }
            NativeHostCommand::CancelPointers { lease: _ } => {
                // TODO(phase 6): cancel current presenter/input-local pointers on retirement.
                Err(not_implemented(
                    NativeControlOperation::CancelPointers,
                    MissingNativeControl::LocalPointerCancellation,
                ))
            }
            NativeHostCommand::Foreground {
                runtime: _,
                active: _,
            } => {
                // TODO(phase 6): apply existing host admission/suspension facts to the actual
                // runtime; suspend rendering/input without changing rules-owned timer semantics.
                Err(not_implemented(
                    NativeControlOperation::Foreground,
                    MissingNativeControl::ForegroundRuntimeAdmission,
                ))
            }
            NativeHostCommand::Back { runtime: _ } => {
                // TODO(phase 6): route to the existing runtime leave flow and acknowledge only
                // an actual accepted leave. This source skeleton never confirms Exit.
                Err(not_implemented(
                    NativeControlOperation::Back,
                    MissingNativeControl::RuntimeLeaveConfirmation,
                ))
            }
            NativeHostCommand::ServiceReply {
                runtime: _,
                request_id,
                code: _,
            } => {
                if request_id <= 0 {
                    return Err(NativeHostError::Invalid(
                        NativeBoundaryError::ServiceRequestIdOutOfRange,
                    ));
                }
                // TODO(phase 6): route only replies to exact admitted host requests/capabilities.
                Err(not_implemented(
                    NativeControlOperation::ServiceReply,
                    MissingNativeControl::ScopedHostServiceReplyRouting,
                ))
            }
        }
    }

    /// Separate synchronous `SurfaceHolder` barrier. Err means the surface was not fenced.
    pub fn fence_surface(&self, lease: SurfaceLease) -> Result<(), NativeHostError> {
        self.backend
            .fence_surface(lease)
            .map_err(NativeHostError::Backend)
    }
}

const fn not_implemented(
    operation: NativeControlOperation,
    missing: MissingNativeControl,
) -> NativeHostError {
    NativeHostError::NotImplemented { operation, missing }
}

#[cfg(test)]
mod tests {
    use std::{
        future::Future,
        pin::pin,
        task::{Context, Poll, Waker},
    };

    use tabula_render_macroquad::embedded::{EmbeddedOperation, MissingEmbeddedMechanism};

    use super::*;

    fn runtime() -> RuntimeId {
        RuntimeId::new(1).unwrap()
    }
    fn lease() -> SurfaceLease {
        SurfaceLease::new(runtime(), 1).unwrap()
    }
    fn game_id() -> GameId {
        GameId::new("com.example.local").unwrap()
    }
    fn preferences() -> NativeLaunchPreferences {
        NativeLaunchPreferences {
            theme: NativeTheme::Dark,
            reduced_motion: true,
            locale: NativeLocale::English,
            request_keep_awake: false,
        }
    }
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
    fn execute(
        host: &NativeGameHostDraft,
        command: NativeHostCommand,
    ) -> Result<(), NativeHostError> {
        let mut future = pin!(host.execute(command));
        let waker = Waker::noop();
        match future.as_mut().poll(&mut Context::from_waker(waker)) {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("source-only unavailable operation unexpectedly waits"),
        }
    }
    fn backend_error(
        operation: EmbeddedOperation,
        missing: MissingEmbeddedMechanism,
    ) -> NativeHostError {
        NativeHostError::Backend(EmbeddedSurfaceError::Unavailable { operation, missing })
    }

    #[test]
    fn no_compiled_game_feature_advertises_a_packaged_native_runtime() {
        assert!(NativeGameHostDraft::default()
            .inventory()
            .packaged_games()
            .is_empty());
        assert_eq!(
            NativeRuntimeInventoryDraft.resolve_selected_runtime(&game_id()),
            Err(NativeHostError::Unavailable {
                game_id: game_id(),
                missing:
                    MissingNativeComposition::PackagedRegistrySelectedLocalRuntimeAndVerifiedAssets
            })
        );
    }

    #[test]
    fn selected_assets_cannot_report_resources_ready() {
        assert_eq!(
            NativeRuntimeInventoryDraft.prepare_selected_assets(&game_id()),
            Err(NativeHostError::Unavailable {
                game_id: game_id(),
                missing: MissingNativeComposition::SelectedVerifiedAssetPackAndContextUpload
            })
        );
    }

    #[test]
    fn launch_uses_core_identity_and_enforces_host_length_bound() {
        let at_limit = GameId::new(format!("a.{}", "b".repeat(78))).unwrap();
        let request = NativeLaunchRequest::new(at_limit.clone(), preferences()).unwrap();
        assert_eq!(request.game_id(), &at_limit);
        assert_eq!(request.preferences(), preferences());
        let too_long = GameId::new(format!("a.{}", "b".repeat(79))).unwrap();
        assert_eq!(
            NativeLaunchRequest::new(too_long.clone(), preferences()),
            Err(NativeBoundaryError::GameIdTooLong)
        );
        assert_eq!(
            NativeRuntimeInventoryDraft.resolve_selected_runtime(&too_long),
            Err(NativeHostError::Invalid(NativeBoundaryError::GameIdTooLong))
        );
        assert_eq!(
            NativeRuntimeInventoryDraft.prepare_selected_assets(&too_long),
            Err(NativeHostError::Invalid(NativeBoundaryError::GameIdTooLong))
        );
        assert!(GameId::new("not-a-canonical-id").is_err());
    }

    #[test]
    fn start_remains_unavailable_for_foreground_and_background() {
        let host = NativeGameHostDraft::default();
        for foreground in [false, true] {
            let request = NativeLaunchRequest::new(game_id(), preferences()).unwrap();
            assert_eq!(execute(&host, NativeHostCommand::Start { runtime: runtime(), request, foreground }), Err(NativeHostError::Unavailable { game_id: game_id(), missing: MissingNativeComposition::PackagedRegistrySelectedLocalRuntimeAndVerifiedAssets }));
        }
        assert!(host.inventory().packaged_games().is_empty());
    }

    #[test]
    fn pointer_constructor_rejects_each_invalid_bound() {
        for id in [-1, 32, i32::MIN, i32::MAX] {
            assert_eq!(
                NativePointer::new(id, NativePointerPhase::Down, 0.0, 0.0),
                Err(NativeBoundaryError::PointerIdOutOfRange)
            );
        }
        for value in [
            -32_769.0,
            32_769.0,
            f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
        ] {
            assert_eq!(
                NativePointer::new(0, NativePointerPhase::Move, value, 0.0),
                Err(NativeBoundaryError::PointerXOutOfRange)
            );
            assert_eq!(
                NativePointer::new(0, NativePointerPhase::Move, 0.0, value),
                Err(NativeBoundaryError::PointerYOutOfRange)
            );
        }
        for (id, x, y) in [(0, -32_768.0, 32_768.0), (31, 32_768.0, -32_768.0)] {
            let input = NativePointer::new(id, NativePointerPhase::Up, x, y).unwrap();
            assert_eq!(input.id(), id);
            assert_eq!(input.phase(), NativePointerPhase::Up);
            assert_eq!(input.position_px().0.to_bits(), x.to_bits());
            assert_eq!(input.position_px().1.to_bits(), y.to_bits());
        }
    }

    #[test]
    fn attach_resize_detach_and_rendering_preserve_backend_unavailability() {
        let host = NativeGameHostDraft::default();
        let geometry = SurfaceGeometry::new(320, 240, 2.0).unwrap();
        for (command, operation, missing) in [
            (
                NativeHostCommand::Attach(proposal()),
                EmbeddedOperation::AttachContext,
                MissingEmbeddedMechanism::ExternalSurfaceContextBootstrap,
            ),
            (
                NativeHostCommand::Resize {
                    lease: lease(),
                    geometry,
                },
                EmbeddedOperation::Resize,
                MissingEmbeddedMechanism::OwnerThreadContextResize,
            ),
            (
                NativeHostCommand::Detach { lease: lease() },
                EmbeddedOperation::Detach,
                MissingEmbeddedMechanism::OwnerThreadSurfaceRelease,
            ),
            (
                NativeHostCommand::Rendering {
                    lease: lease(),
                    enabled: false,
                },
                EmbeddedOperation::Rendering,
                MissingEmbeddedMechanism::SuspendedFrameScheduling,
            ),
            (
                NativeHostCommand::Rendering {
                    lease: lease(),
                    enabled: true,
                },
                EmbeddedOperation::Rendering,
                MissingEmbeddedMechanism::SuspendedFrameScheduling,
            ),
        ] {
            assert_eq!(
                execute(&host, command),
                Err(backend_error(operation, missing))
            );
        }
    }

    #[test]
    fn runtime_control_stubs_identify_each_missing_mechanism() {
        let host = NativeGameHostDraft::default();
        for (command, operation, missing) in [
            (
                NativeHostCommand::Pointer {
                    lease: lease(),
                    input: NativePointer::new(0, NativePointerPhase::Down, 1.0, 2.0).unwrap(),
                },
                NativeControlOperation::Pointer,
                MissingNativeControl::OwnerThreadInputAdmissionAndDensityConversion,
            ),
            (
                NativeHostCommand::CancelPointers { lease: lease() },
                NativeControlOperation::CancelPointers,
                MissingNativeControl::LocalPointerCancellation,
            ),
            (
                NativeHostCommand::Foreground {
                    runtime: runtime(),
                    active: false,
                },
                NativeControlOperation::Foreground,
                MissingNativeControl::ForegroundRuntimeAdmission,
            ),
            (
                NativeHostCommand::Foreground {
                    runtime: runtime(),
                    active: true,
                },
                NativeControlOperation::Foreground,
                MissingNativeControl::ForegroundRuntimeAdmission,
            ),
            (
                NativeHostCommand::Back { runtime: runtime() },
                NativeControlOperation::Back,
                MissingNativeControl::RuntimeLeaveConfirmation,
            ),
            (
                NativeHostCommand::ServiceReply {
                    runtime: runtime(),
                    request_id: 1,
                    code: NativeServiceReplyCode::Denied,
                },
                NativeControlOperation::ServiceReply,
                MissingNativeControl::ScopedHostServiceReplyRouting,
            ),
        ] {
            assert_eq!(
                execute(&host, command),
                Err(not_implemented(operation, missing))
            );
        }
    }

    #[test]
    fn service_reply_rejects_invalid_ids_before_unavailable_dispatch() {
        let host = NativeGameHostDraft::default();
        for request_id in [i32::MIN, -1, 0] {
            assert_eq!(
                execute(
                    &host,
                    NativeHostCommand::ServiceReply {
                        runtime: runtime(),
                        request_id,
                        code: NativeServiceReplyCode::Ok
                    }
                ),
                Err(NativeHostError::Invalid(
                    NativeBoundaryError::ServiceRequestIdOutOfRange
                ))
            );
        }
        for request_id in [1, i32::MAX] {
            assert_eq!(
                execute(
                    &host,
                    NativeHostCommand::ServiceReply {
                        runtime: runtime(),
                        request_id,
                        code: NativeServiceReplyCode::Unsupported
                    }
                ),
                Err(not_implemented(
                    NativeControlOperation::ServiceReply,
                    MissingNativeControl::ScopedHostServiceReplyRouting
                ))
            );
        }
    }

    #[test]
    fn synchronous_fence_and_async_stop_never_claim_completion() {
        let host = NativeGameHostDraft::default();
        assert_eq!(
            host.fence_surface(lease()),
            Err(backend_error(
                EmbeddedOperation::FenceSurface,
                MissingEmbeddedMechanism::SynchronousSurfaceAccessBarrier
            ))
        );
        assert_eq!(
            execute(&host, NativeHostCommand::Stop { runtime: runtime() }),
            Err(backend_error(
                EmbeddedOperation::StopAndJoin,
                MissingEmbeddedMechanism::RetainedWorkerJoinAndContextTeardown
            ))
        );
        assert!(host.inventory().packaged_games().is_empty());
        assert!(execute(&host, NativeHostCommand::Attach(proposal())).is_err());
    }
}
