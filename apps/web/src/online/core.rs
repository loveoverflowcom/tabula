//! Admission UI decisions. No client seat authority or queue is introduced (ADR-0041).
use tabula_match_http::MatchAdmission;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Action {
    Create,
    Join,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Phase {
    Ready,
    Pending(Action),
    Created,
    Joined,
    Error(&'static str),
    Unknown,
    HandoffFailed,
}
impl Phase {
    pub(super) const fn label_key(self) -> &'static str {
        match self {
            Self::Ready => "online.ready",
            Self::Pending(Action::Create) => "online.creating",
            Self::Pending(Action::Join) => "online.joining",
            Self::Created => "online.waiting",
            Self::Joined => "online.joined",
            Self::Error(key) => key,
            Self::Unknown => "online.unknown",
            Self::HandoffFailed => "online.handoff_failed",
        }
    }
}

#[derive(Clone)]
pub(super) struct EntryState {
    pub(super) code: String,
    pub(super) phase: Phase,
    pub(super) admission: Option<MatchAdmission>,
    pub(super) cleanup_failed: bool,
    revision: u64,
    pending: Option<(u64, Action)>,
    dispatched: bool,
}
impl Default for EntryState {
    fn default() -> Self {
        Self {
            code: String::new(),
            phase: Phase::Ready,
            admission: None,
            cleanup_failed: false,
            revision: 0,
            pending: None,
            dispatched: false,
        }
    }
}
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
impl EntryState {
    pub(super) const fn busy(&self) -> bool {
        self.pending.is_some()
    }
    pub(super) fn can_submit(&self, action: Action, authenticated: bool, blocked: bool) -> bool {
        authenticated
            && !blocked
            && !self.busy()
            && self.admission.is_none()
            && (action == Action::Create || !self.code.trim().is_empty())
    }
    pub(super) fn edit_code(&mut self, raw: &str) {
        if !self.busy() {
            // Bound paste and synthetic input too; preserve invalid characters so
            // the server's existing public-safe code policy remains the authority.
            self.code = raw
                .trim()
                .chars()
                .take(12)
                .collect::<String>()
                .to_ascii_uppercase();
            if self.phase == Phase::Error("online.invalid_code") {
                self.phase = Phase::Ready;
            }
        }
    }
    pub(super) fn begin(
        &mut self,
        action: Action,
        authenticated: bool,
        blocked: bool,
    ) -> Option<u64> {
        if !self.can_submit(action, authenticated, blocked) {
            return None;
        }
        self.revision = self.revision.checked_add(1)?;
        self.pending = Some((self.revision, action));
        self.dispatched = false;
        self.phase = Phase::Pending(action);
        Some(self.revision)
    }
    pub(super) fn result_key(&self, blocked: bool, cleanup_failed: bool) -> &'static str {
        if blocked && !self.busy() && !cleanup_failed {
            "online.unknown"
        } else {
            self.phase.label_key()
        }
    }
    pub(super) fn was_dispatched(&self, revision: u64) -> bool {
        self.current(revision) && self.dispatched
    }
    pub(super) fn current(&self, revision: u64) -> bool {
        self.pending.is_some_and(|pending| pending.0 == revision)
    }
    pub(super) fn dispatched(&mut self, revision: u64) -> bool {
        if !self.current(revision) {
            return false;
        }
        self.dispatched = true;
        true
    }
    pub(super) fn complete(
        &mut self,
        revision: u64,
        result: Result<MatchAdmission, &'static str>,
    ) -> bool {
        let Some((_, action)) = self.pending.filter(|pending| pending.0 == revision) else {
            return false;
        };
        self.pending = None;
        self.phase = match result {
            Ok(admission) => {
                self.admission = Some(admission);
                match action {
                    Action::Create => Phase::Created,
                    Action::Join => Phase::Joined,
                }
            }
            Err("online.unknown") => Phase::Unknown,
            Err(key) => Phase::Error(key),
        };
        self.dispatched = false;
        true
    }
    /// Retire before aborting. Aborting a dispatched POST is never a cancellation receipt.
    pub(super) fn retire(&mut self) -> bool {
        if self.pending.is_none() {
            return false;
        }
        let unknown = self.dispatched;
        self.pending = None;
        self.dispatched = false;
        self.phase = if unknown {
            Phase::Unknown
        } else {
            Phase::Ready
        };
        unknown
    }
    pub(super) fn handoff_failed(&mut self) {
        if self.admission.is_some() {
            self.phase = Phase::HandoffFailed;
        }
    }
    pub(super) fn clear_admission(&mut self) {
        let had_admission = self.admission.take().is_some();
        self.cleanup_failed = false;
        if matches!(
            self.phase,
            Phase::Created | Phase::Joined | Phase::HandoffFailed
        ) || (had_admission && matches!(self.phase, Phase::Error(_)))
        {
            self.phase = Phase::Ready;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn admission() -> MatchAdmission {
        MatchAdmission::new(
            "00000000000000000000000000000001".into(),
            "test.module".into(),
            "1.0.0".into(),
            0,
            Some("ABCD2345EFGH".into()),
            false,
        )
        .unwrap()
    }
    #[test]
    fn duplicate_click_and_enter_share_one_pending_gate() {
        let mut state = EntryState::default();
        state.edit_code("abcd2345efgh");
        let first = state.begin(Action::Create, true, false).unwrap();
        assert_eq!(state.begin(Action::Create, true, false), None);
        assert_eq!(state.begin(Action::Join, true, false), None);
        state.edit_code("another");
        assert_eq!(state.code, "ABCD2345EFGH");
        assert!(state.complete(first, Ok(admission())));
        assert_eq!(state.phase, Phase::Created);
        assert_eq!(state.begin(Action::Create, true, false), None);
        assert_eq!(state.begin(Action::Join, true, false), None);
    }
    #[test]
    fn bounded_uppercase_paste_preserves_invalid_input_for_safe_denial() {
        let mut state = EntryState::default();
        for (raw, expected) in [
            (" abcd2345efgh ", "ABCD2345EFGH"),
            ("abcdefghijklmn", "ABCDEFGHIJKL"),
            ("abc! ?", "ABC! ?"),
            ("   ", ""),
        ] {
            state.edit_code(raw);
            assert_eq!(state.code, expected);
        }
        assert_eq!(state.begin(Action::Join, true, false), None);
        assert_eq!(state.begin(Action::Create, false, false), None);
    }
    #[test]
    fn context_failure_is_retryable_but_uncertain_mutation_is_not() {
        let mut state = EntryState::default();
        let first = state.begin(Action::Create, true, false).unwrap();
        assert!(!state.retire(), "context-only work did not mutate anything");
        assert!(
            !state.complete(first, Ok(admission())),
            "stale result retired"
        );
        let second = state.begin(Action::Create, true, false).unwrap();
        assert!(state.dispatched(second));
        let blocked = state.retire();
        assert!(blocked);
        assert_eq!(state.phase, Phase::Unknown);
        for action in [Action::Create, Action::Join] {
            assert_eq!(state.begin(action, true, blocked), None);
        }
        assert!(!state.complete(second, Ok(admission())));
    }
    #[test]
    fn late_result_cannot_finish_a_new_operation_after_context_or_route_change() {
        let mut state = EntryState::default();
        let old = state.begin(Action::Create, true, false).unwrap();
        state.retire();
        let new = state.begin(Action::Create, true, false).unwrap();
        assert_ne!(old, new);
        assert!(!state.dispatched(old));
        assert!(!state.complete(old, Err("online.unavailable")));
        assert!(state.current(new));
        assert_eq!(state.phase, Phase::Pending(Action::Create));
    }
    #[test]
    fn local_latch_cleanup_failure_does_not_turn_known_receipts_into_unknown_outcomes() {
        let mut state = EntryState::default();
        let revision = state.begin(Action::Create, true, false).unwrap();
        state.dispatched(revision);
        assert!(state.was_dispatched(revision));
        assert!(state.complete(revision, Ok(admission())));
        assert_eq!(state.result_key(true, true), "online.waiting");
        assert_eq!(state.begin(Action::Create, true, true), None);
        let mut denied = EntryState::default();
        denied.edit_code("bad-code");
        let revision = denied.begin(Action::Join, true, false).unwrap();
        denied.complete(revision, Err("online.invalid_code"));
        assert_eq!(denied.result_key(true, true), "online.invalid_code");
        assert_eq!(
            denied.result_key(true, false),
            "online.unknown",
            "a reload has no confirmed in-memory receipt"
        );
    }
    #[test]
    fn account_change_clears_receipt_and_its_completed_copy_but_preserves_uncertainty() {
        for phase in [
            Phase::Created,
            Phase::Joined,
            Phase::HandoffFailed,
            Phase::Error("online.incompatible"),
        ] {
            let mut state = EntryState::default();
            state.edit_code("abcd2345efgh");
            state.admission = Some(admission());
            state.phase = phase;
            state.clear_admission();
            assert!(state.admission.is_none());
            assert_eq!(state.phase, Phase::Ready);
            assert_eq!(state.code, "ABCD2345EFGH");
            assert!(state.can_submit(Action::Create, true, false));
        }
        let mut state = EntryState::default();
        let revision = state.begin(Action::Create, true, false).unwrap();
        state.dispatched(revision);
        assert!(state.retire());
        state.clear_admission();
        assert_eq!(state.phase, Phase::Unknown);
        assert_eq!(state.begin(Action::Create, true, true), None);
    }
    #[test]
    fn refused_document_navigation_keeps_the_known_match_for_explicit_retry() {
        let mut state = EntryState::default();
        let revision = state.begin(Action::Create, true, false).unwrap();
        state.complete(revision, Ok(admission()));
        state.handoff_failed();
        assert_eq!(state.phase, Phase::HandoffFailed);
        assert_eq!(
            state.admission.as_ref().unwrap().join_code(),
            Some("ABCD2345EFGH")
        );
        assert_eq!(state.begin(Action::Create, true, false), None);
        state.retire();
        assert_eq!(
            state.phase,
            Phase::HandoffFailed,
            "Back does not silently launch again"
        );
        state.clear_admission();
        state.handoff_failed();
        assert_eq!(state.phase, Phase::Ready, "no phantom failed match");
    }
    #[test]
    fn safe_join_denial_keeps_code_and_allows_explicit_corrected_submit() {
        let mut state = EntryState::default();
        state.edit_code("bad-code");
        let revision = state.begin(Action::Join, true, false).unwrap();
        state.dispatched(revision);
        assert!(state.complete(revision, Err("online.invalid_code")));
        assert_eq!(state.code, "BAD-CODE");
        assert_eq!(state.phase, Phase::Error("online.invalid_code"));
        state.edit_code("abcd2345efgh");
        assert_eq!(state.phase, Phase::Ready);
        let next = state.begin(Action::Join, true, false).unwrap();
        assert!(state.complete(next, Ok(admission())));
        assert_eq!(
            state.phase,
            Phase::Joined,
            "action, not room readiness, owns the copy"
        );
    }
}
