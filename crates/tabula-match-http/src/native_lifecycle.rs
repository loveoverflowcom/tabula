//! Bounded local capacity and owner drain controls (doc03 §6.4; ADR-0041).
use std::{fmt, time::Duration};
use tabula_match::runtime::{AdmissionError, Exit, HostControl, Summary};
use tokio::{sync::watch, task::JoinHandle, time::Instant};

/// Explicit local gateway budgets, within the reviewed isolated hard caps.
///
/// This does not change durable room/history or operation-ledger retention.
/// The live-owner budget is process-local; PR02 owns reusable room retirement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MatchHttpConfig {
    requests: usize,
    work: usize,
    live: usize,
}
impl MatchHttpConfig {
    /// Validate capacities before constructing any semaphore or gateway.
    pub fn new(requests: usize, work: usize, live: usize) -> Result<Self, InvalidMatchHttpConfig> {
        if !(1..=64).contains(&requests) || !(1..=64).contains(&work) || !(1..=128).contains(&live)
        {
            return Err(InvalidMatchHttpConfig);
        }
        Ok(Self {
            requests,
            work,
            live,
        })
    }
    /// Maximum concurrently admitted HTTP requests (doc03 §3.2).
    pub const fn request_capacity(self) -> usize {
        self.requests
    }
    /// Maximum owned journal operations, including cancelled HTTP requesters.
    pub const fn work_capacity(self) -> usize {
        self.work
    }
    /// Maximum retained process-local match owners; no historical row is evicted.
    pub const fn live_capacity(self) -> usize {
        self.live
    }
}
impl Default for MatchHttpConfig {
    fn default() -> Self {
        Self {
            requests: 64,
            work: 64,
            live: 128,
        }
    }
}
/// A gateway budget was zero or exceeded its reviewed isolated hard cap.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidMatchHttpConfig;
impl fmt::Display for InvalidMatchHttpConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("match gateway capacities require requests/work in 1..=64 and live in 1..=128")
    }
}
impl std::error::Error for InvalidMatchHttpConfig {}

/// Bounded shutdown evidence, never a durable recovery cursor (doc03 §6.4).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ShutdownReport {
    /// Owners observed exiting through the actor's ordered Drain input.
    pub drained: usize,
    /// Owners observed failing or completing cleanup unsuccessfully.
    pub failed: usize,
    /// Owners still retained because the overall deadline expired.
    pub remaining: usize,
    /// Request/work/owner quiescence did not complete within the caller budget.
    pub timed_out: bool,
}
impl ShutdownReport {
    /// All owners drained successfully and all admitted request/work permits returned.
    pub const fn is_complete(self) -> bool {
        !self.timed_out && self.failed == 0 && self.remaining == 0
    }
}

pub(super) struct OwnerLifecycle {
    host: HostControl,
    done: watch::Receiver<Option<Result<Exit, ()>>>,
}
/// Cleanup precedes completion publication, including cancellation/panic exits.
pub(super) fn track_owner(
    host: HostControl,
    task: JoinHandle<Summary>,
    cleanup: impl FnOnce() -> bool + Send + 'static,
) -> OwnerLifecycle {
    let (finished, done) = watch::channel(None);
    tokio::spawn(async move {
        let result = task.await.map(|summary| summary.exit).map_err(|_| ());
        let result = if cleanup() { result } else { Err(()) };
        let _ = finished.send(Some(result));
    });
    OwnerLifecycle { host, done }
}
impl OwnerLifecycle {
    /// Retry a full mailbox without dropping its owner; completion is observed
    /// after the owner task and its authority/output cleanup have both finished.
    pub async fn drain(&self, deadline: Instant) -> Result<bool, ()> {
        tokio::time::timeout_at(deadline, async {
            let mut done = self.done.clone();
            if done.borrow().is_none() {
                loop {
                    match self.host.drain() {
                        Ok(ticket) => {
                            let _ = ticket.wait().await;
                            break;
                        }
                        Err(AdmissionError::Busy) => {
                            tokio::time::sleep(Duration::from_millis(10)).await;
                        }
                        Err(_) => break,
                    }
                    if done.borrow().is_some() {
                        break;
                    }
                }
            }
            loop {
                if let Some(result) = *done.borrow_and_update() {
                    return Ok(matches!(result, Ok(Exit::Drained)));
                }
                if done.changed().await.is_err() {
                    return Ok(false);
                }
            }
        })
        .await
        .map_err(|_| ())?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    };
    use tabula_core::{
        InputIndex, MatchId, MatchSeed, Occupant, SeatEntry, SeatId, SeatRoster, UserId,
    };
    use tabula_game_api::Effect;
    use tabula_match::{
        durable::{Journal, JournalRecord, RuntimePortError},
        ports::Clock,
        runtime::{self, Binding, Ports},
        runtime_ports::{Authority, AuthorityLost, Effects, Output, Purpose},
    };
    use tabula_protocol::ServerEnvelope;
    use tokio::sync::{oneshot, Mutex as AsyncMutex};

    struct OpenAuthority;
    impl Authority for OpenAuthority {
        fn with_current<T>(
            &self,
            _: &Binding,
            _: Purpose,
            action: impl FnOnce() -> T,
        ) -> Result<T, AuthorityLost> {
            Ok(action())
        }
    }
    struct PausedJournal {
        entered: Mutex<Option<oneshot::Sender<()>>>,
        release: AsyncMutex<Option<oneshot::Receiver<()>>>,
        commits: AtomicUsize,
    }
    impl Journal for PausedJournal {
        async fn append(&self, _: JournalRecord) -> Result<(), RuntimePortError> {
            if let Some(entered) = self.entered.lock().unwrap().take() {
                entered.send(()).unwrap();
            }
            if let Some(release) = self.release.lock().await.take() {
                release.await.unwrap();
            }
            self.commits.fetch_add(1, Ordering::AcqRel);
            Ok(())
        }
    }
    struct ClosedOutput;
    impl Output for ClosedOutput {
        fn submit(&self, _: &Binding, _: ServerEnvelope) -> Result<(), RuntimePortError> {
            panic!("unattached owner cannot publish")
        }
    }
    struct NoEffects;
    impl Effects for NoEffects {
        async fn execute(
            &self,
            _: MatchId,
            _: InputIndex,
            _: Vec<Effect>,
        ) -> Result<(), RuntimePortError> {
            Ok(())
        }
    }
    struct FixedClock;
    impl Clock for FixedClock {
        fn now_unix_ms(&self) -> u64 {
            1
        }
        fn monotonic_ms(&self) -> u64 {
            0
        }
    }

    #[tokio::test]
    async fn owner_drain_waits_for_commit_and_cleanup_timeout_preserves_owner() {
        let game = tabula_registry::registered_games()
            .into_iter()
            .find(|game| game.direct_host_supported())
            .expect("linked approved direct module");
        let config = game
            .normalize_direct(2, &tabula_registry::ConfigDraft::with_defaults(game.form()))
            .unwrap();
        let roster = SeatRoster::new(
            (0..2)
                .map(|seat| SeatEntry {
                    seat: SeatId(seat),
                    occupant: Occupant::Human(UserId(u128::from(seat) + 1)),
                    team: None,
                })
                .collect(),
        )
        .unwrap();
        let created = game
            .create_match(
                config.canonical_config(),
                &roster,
                MatchSeed::from_bytes([11; 32]),
            )
            .unwrap();
        let (entered, started) = oneshot::channel();
        let (release, commit_barrier) = oneshot::channel();
        let journal = Arc::new(PausedJournal {
            entered: Mutex::new(Some(entered)),
            release: AsyncMutex::new(Some(commit_barrier)),
            commits: AtomicUsize::new(0),
        });
        let (handle, host, task) = runtime::spawn(
            MatchId(881),
            created,
            Ports {
                authority: Arc::new(OpenAuthority),
                journal: journal.clone(),
                output: Arc::new(ClosedOutput),
                effects: Arc::new(NoEffects),
                clock: Arc::new(FixedClock),
            },
            runtime::Limits::default(),
        )
        .unwrap();
        let owner = task.abort_handle();
        let cleaned = Arc::new(AtomicBool::new(false));
        let observed = cleaned.clone();
        let lifecycle = track_owner(host, task, move || {
            observed.store(true, Ordering::Release);
            true
        });
        started.await.unwrap();
        assert_eq!(
            lifecycle
                .drain(Instant::now() + Duration::from_millis(5))
                .await,
            Err(())
        );
        assert!(!handle.is_closed());
        assert!(!owner.is_finished());
        assert!(!cleaned.load(Ordering::Acquire));
        assert_eq!(journal.commits.load(Ordering::Acquire), 0);
        release.send(()).unwrap();
        assert_eq!(
            lifecycle
                .drain(Instant::now() + Duration::from_secs(1))
                .await,
            Ok(true)
        );
        assert!(handle.is_closed());
        assert!(owner.is_finished());
        assert!(cleaned.load(Ordering::Acquire));
        assert_eq!(journal.commits.load(Ordering::Acquire), 1);
    }

    #[test]
    fn capacities_reject_zero_and_reviewed_cap_overflow() {
        for requests in [0, 65, usize::MAX] {
            assert_eq!(
                MatchHttpConfig::new(requests, 1, 1),
                Err(InvalidMatchHttpConfig)
            );
        }
        for work in [0, 65, usize::MAX] {
            assert_eq!(
                MatchHttpConfig::new(1, work, 1),
                Err(InvalidMatchHttpConfig)
            );
        }
        for live in [0, 129, usize::MAX] {
            assert_eq!(
                MatchHttpConfig::new(1, 1, live),
                Err(InvalidMatchHttpConfig)
            );
        }
        let small = MatchHttpConfig::new(1, 1, 1).unwrap();
        assert_eq!(small.request_capacity(), 1);
        assert_eq!(small.work_capacity(), 1);
        assert_eq!(small.live_capacity(), 1);
        assert_eq!(
            MatchHttpConfig::new(64, 64, 128).unwrap(),
            MatchHttpConfig::default()
        );
    }

    #[test]
    fn incomplete_shutdown_never_claims_success() {
        assert!(ShutdownReport::default().is_complete());
        for report in [
            ShutdownReport {
                failed: 1,
                ..ShutdownReport::default()
            },
            ShutdownReport {
                remaining: 1,
                ..ShutdownReport::default()
            },
            ShutdownReport {
                timed_out: true,
                ..ShutdownReport::default()
            },
        ] {
            assert!(!report.is_complete());
        }
    }
}
