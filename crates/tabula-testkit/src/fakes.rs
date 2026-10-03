//! In-memory implementations of the match runtime's ports.
//!
//! # Why these exist
//!
//! `tabula-match` is the hardest, most correctness-critical async code in the
//! product, and it must be testable **with no database and no HTTP server**
//! (doc 01 §3). That is only true if every port has a fast, deterministic fake.
//! These are those fakes.
//!
//! A test that needs Postgres to check ordering logic is a test that will be
//! disabled the first time CI is slow.
//!
//! # Phase
//!
//! **Phase 4.** The port traits live in `tabula-match`, which does not become
//! real until then. This module is gated behind the `runtime` feature so a
//! Phase-0 checkout does not carry them.

#![cfg(feature = "runtime")]

// TODO(phase 4): implement against the traits in `tabula_match::ports`:
//
//   FakeEventLog       Vec<(InputIndex, Vec<Bytes>)>, no batching, no I/O.
//                      Must expose an injectable "commit fails" mode — the
//                      AckAfterPersist path is only correct if the failure case
//                      is tested, and it never happens by accident.
//
//   FakeSnapshotStore  BTreeMap<StateVersion, Bytes>. `load_nearest(v)` must
//                      return the newest snapshot <= v (doc 03 §9.1).
//
//   FakeMatchRepo      In-memory `matches` + `match_players`. Must enforce the
//                      `ended_at IS NULL` guard so EndMatch idempotency is
//                      genuinely exercised (doc 03 §7.1).
//
//   FakeClock          Manually advanced. This is what makes timer, grace-period,
//                      and hibernation tests run in microseconds instead of
//                      minutes — and what makes them deterministic.
//
//   FakeBotRunner      Returns a scripted move, or a configurable delay, so the
//                      `(match_id, seat, state_version)` idempotency key can be
//                      tested by replaying a stale bot move.
//
//   RecordingBroadcast Captures per-viewer-group byte streams so a test can
//                      assert what each seat and each spectator actually
//                      received. This is the fake the projection tests use to
//                      check leaks at the socket boundary rather than at the
//                      function boundary — doc 08 §5 requires exactly that for
//                      werewolf chat scopes.
