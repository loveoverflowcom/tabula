//! Explicit disposable-test barriers; absent from ordinary isolated builds.
use std::{future::Future, pin::Pin};
use tabula_core::MatchId;
use tabula_match::durable::RuntimePortError;
use tabula_session::AuthSessionId;

/// A real command boundary, without exposing payloads or canonical state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AcceptanceFaultPhase {
    BeforeSubmission,
    BeforeCommit,
    AfterCommit,
}

/// Trusted in-process correlation for a disposable acceptance barrier.
#[derive(Clone, Debug)]
pub struct AcceptanceFaultPoint {
    pub match_id: MatchId,
    pub record: AuthSessionId,
    pub attachment_id: Option<String>,
    pub phase: AcceptanceFaultPhase,
}

/// Explicit test hook; implementations must bound their wait and never log secrets.
pub trait AcceptanceFaultHook: Send + Sync + 'static {
    fn reach(
        &self,
        point: AcceptanceFaultPoint,
    ) -> Pin<Box<dyn Future<Output = Result<(), RuntimePortError>> + Send + '_>>;
}
