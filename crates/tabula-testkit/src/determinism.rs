//! Determinism and transactionality harnesses. (I-2, I-7, I-8, contract R2, R8)
//!
//! ## What "deterministic" means here, precisely
//!
//! ```text
//! same initial state (from the same MatchSeed and MatchConfig)
//! + same ordered input sequence
//! + same rules version
//! ================================================================
//! byte-identical final state, identical event sequence, identical hashes
//! ```
//!
//! And it must hold **across** process restarts, machines, operating systems,
//! architectures (x86-64 and aarch64), native and WASM, debug and release.
//! (doc 00 §5.1)
//!
//! ## Everything here compares canonical bytes, never `Debug`
//!
//! A `Debug`-based comparison passes while the stored bytes differ, which is
//! exactly the failure mode these harnesses exist to catch (doc 05 §7.1). It also
//! matters for games that override `state_hash` with an incremental structure
//! (doc 02 §12.4, tiles): an incremental hash can miss a mutation that the
//! canonical encoding shows. So the ground truth is
//! [`tabula_core::canonical_encode`], and the hash is reported alongside it to
//! name the divergence point.
//!
//! ## Why the R2 checker is not opt-in
//!
//! `apply` takes `&mut State`, so purity is a contract rather than a type
//! guarantee (doc 02 §3.3, ADR-026 §1). That trade is only acceptable because
//! the violation is caught mechanically for every game, on every rejected input,
//! without the author opting in. A rejected input that mutated state corrupts the
//! match silently and surfaces weeks later as a replay divergence.
//!
//! ## The RNG index convention (pinned here, Phase 4 must match)
//!
//! ```text
//! create      draws from DetRng::for_input(seed, InputIndex(0))
//! input i     draws from DetRng::for_input(seed, InputIndex(i + 1))
//! ```
//!
//! `create` gets its own index so it cannot share a stream with the first input.
//! Because each index derives an independent stream, the number of draws made
//! while applying one input cannot shift any later input — which is what makes a
//! rejection a total no-op with no rollback machinery (contract R8, ADR-026 §5).
//!
//! What the runtime may choose freely is whether a *rejected* input consumes an
//! index. What it may not do is choose differently on replay than it did live.

use tabula_core::{
    canonical_decode, canonical_encode, InputIndex, LogicalTime, MatchSeed, RuleErrorCode,
    SeatRoster, StateHash, StateVersion,
};
use tabula_game_api::{Budget, Ctx, GameRules, Input};

/// Everything needed to run one match deterministically.
///
/// This is the whole input side of the determinism contract: same `Scenario`,
/// same everything. Nothing else may influence the result — if a harness needs a
/// field that is not here, that field is a determinism hole.
pub struct Scenario<R: GameRules> {
    pub config: R::Config,
    pub roster: SeatRoster,
    pub seed: MatchSeed,
    pub inputs: Vec<Input<R::Command>>,
}

// Manual because `R::Config`/`R::Command` carry no `Debug` bound, and because a
// derived impl would print the seed — a seed in a log line is a leaked deck
// (doc 06 §9.3).
impl<R: GameRules> core::fmt::Debug for Scenario<R> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Scenario")
            .field("seats", &self.roster.len())
            .field("inputs", &self.inputs.len())
            .finish_non_exhaustive()
    }
}

/// Result of running one input sequence to completion.
#[derive(Debug, PartialEq, Eq)]
pub struct RunTrace {
    pub final_hash: StateHash,
    /// Canonical bytes of the final state. The ground truth for comparison —
    /// see the module note on why this is not the hash alone.
    pub final_state: Vec<u8>,
    pub final_version: StateVersion,
    /// Hash after every input, so a divergence report can name the exact index.
    pub checkpoints: Vec<(u64, StateHash)>,
    /// Canonically encoded, in emission order. Order is part of the contract.
    pub events_encoded: Vec<Vec<u8>>,
    pub effects_encoded: Vec<Vec<u8>>,
    pub rejections: Vec<(u64, RuleErrorCode)>,
}

/// A game rejected its own opening position.
#[derive(Debug, thiserror::Error)]
#[error("create() failed for this scenario: {0}")]
pub struct ScenarioFailed(String);

/// Run a scenario the way the match runtime will (doc 03 §7), and record
/// everything the determinism contract covers.
///
/// The reference implementation of the transition loop for Phase 0. It is
/// deliberately tiny: anything it needs that is not in [`Scenario`] would be a
/// determinism hole, so keeping it small keeps the contract honest.
///
/// # Errors
/// [`ScenarioFailed`] if `create` rejects the config/roster.
pub fn run<R: GameRules>(scenario: &Scenario<R>) -> Result<RunTrace, ScenarioFailed> {
    let mut rng = tabula_core::DetRng::for_input(&scenario.seed, InputIndex(0));
    let mut ctx = Ctx {
        now: LogicalTime::ZERO,
        index: InputIndex(0),
        rng: &mut rng,
        budget: Budget {
            max_apply_micros: u32::MAX,
            max_events_per_input: u16::MAX,
        },
    };

    let init = R::create(&scenario.config, &scenario.roster, &mut ctx)
        .map_err(|e| ScenarioFailed(format!("{e:?}")))?;

    let mut state = init.state;
    let mut version = StateVersion(0);
    let mut events_encoded = Vec::new();
    let mut effects_encoded = Vec::new();
    let mut checkpoints = Vec::new();
    let mut rejections = Vec::new();

    for event in &init.events {
        events_encoded.push(encode(event));
    }
    for effect in &init.effects {
        effects_encoded.push(encode(effect));
    }

    for (i, input) in scenario.inputs.iter().enumerate() {
        let index = InputIndex(i as u64 + 1);
        let mut rng = tabula_core::DetRng::for_input(&scenario.seed, index);
        let mut ctx = Ctx {
            // Logical time advances with the input position. Real time never
            // enters — the runtime records when a timer fired, rules only read
            // what the log says (I-3).
            now: LogicalTime(index.0 * 1_000),
            index,
            rng: &mut rng,
            budget: Budget {
                max_apply_micros: u32::MAX,
                max_events_per_input: u16::MAX,
            },
        };

        match R::apply(&mut state, input.clone(), &mut ctx) {
            Ok(outcome) => {
                // I-7: +1 per successfully applied input, and never otherwise.
                version = StateVersion(version.0 + 1);
                for event in &outcome.events {
                    events_encoded.push(encode(event));
                }
                for effect in &outcome.effects {
                    effects_encoded.push(encode(effect));
                }
            }
            Err(e) => rejections.push((index.0, e.code)),
        }

        checkpoints.push((index.0, R::state_hash(&state)));
    }

    Ok(RunTrace {
        final_hash: R::state_hash(&state),
        final_state: encode(&state),
        final_version: version,
        checkpoints,
        events_encoded,
        effects_encoded,
        rejections,
    })
}

fn encode<T: serde::Serialize>(value: &T) -> Vec<u8> {
    canonical_encode(value).expect("canonical types must be encodable (doc 05 §7.1)")
}

/// Run a sequence twice and assert byte-identical results. (I-2)
///
/// The two runs build state from scratch independently, which is what makes this
/// catch a `HashMap` in canonical state: two independently constructed maps in
/// one thread get different `RandomState` seeds, so their iteration orders — and
/// therefore their Postcard encodings — differ.
///
/// # Panics
/// On any divergence, naming the first differing input index and both hashes.
pub fn assert_deterministic<R: GameRules>(scenario: &Scenario<R>) {
    let Ok(a) = run::<R>(scenario) else { return };
    let b = run::<R>(scenario).expect("create succeeded once, so it must succeed again");

    if let Some((index, left, right)) = first_divergence(&a, &b) {
        panic!(
            "I-2 violated: two runs of the same scenario diverged at input {index}\n  \
             run A hash: {left:02x?}\n  run B hash: {right:02x?}\n\
             Usual causes: HashMap/HashSet in canonical state, a float, wall-clock \
             access, or unordered iteration feeding the event list."
        );
    }

    assert_eq!(
        a.events_encoded, b.events_encoded,
        "I-2 violated: event stream differed between two runs of the same scenario. \
         Event ORDER is part of the contract (R7) — do not build events by iterating \
         an unordered collection."
    );
    assert_eq!(
        a.effects_encoded, b.effects_encoded,
        "I-2 violated: effect stream differed between two runs of the same scenario"
    );
    assert_eq!(
        a.rejections, b.rejections,
        "I-2 violated: the same input was accepted in one run and rejected in the other"
    );
    assert_eq!(
        a.final_state, b.final_state,
        "I-2 violated: final state bytes"
    );
}

fn first_divergence(a: &RunTrace, b: &RunTrace) -> Option<(u64, [u8; 32], [u8; 32])> {
    a.checkpoints
        .iter()
        .zip(&b.checkpoints)
        .find(|((_, x), (_, y))| x != y)
        .map(|((i, x), (_, y))| (*i, x.0, y.0))
}

/// Snapshot mid-run, restore, continue — must land on the same final hash. (I-8)
///
/// `at` is clamped into range, so a proptest may pick it freely. Picking it
/// randomly matters: a fixed midpoint misses the bugs that only appear when the
/// snapshot lands between two halves of a phase transition.
///
/// # Panics
/// If the restored run diverges from the uninterrupted one.
pub fn assert_deterministic_across_snapshot<R: GameRules>(scenario: &Scenario<R>, at: usize) {
    let Ok(whole) = run::<R>(scenario) else {
        return;
    };
    if scenario.inputs.is_empty() {
        return;
    }
    let at = at % scenario.inputs.len();

    // Run the prefix, snapshot through the canonical encoding — the same path
    // production takes into `match_snapshots.payload` — then restore and finish.
    let head = Scenario::<R> {
        config: scenario.config.clone(),
        roster: scenario.roster.clone(),
        seed: scenario.seed.clone(),
        inputs: scenario.inputs[..at].to_vec(),
    };
    let head_trace = run::<R>(&head).expect("prefix of a runnable scenario must run");

    let restored: R::State = canonical_decode(&head_trace.final_state)
        .expect("I-8 violated: a snapshot this crate wrote could not be read back");

    let resumed = continue_from::<R>(scenario, restored, at, head_trace.final_version);

    assert_eq!(
        whole.final_state, resumed.final_state,
        "I-8 violated: snapshot at input {at} then resume produced a different final \
         state than an uninterrupted run. Something outside State is carrying \
         match progress across inputs."
    );
    assert_eq!(
        whole.final_hash, resumed.final_hash,
        "I-8 violated: final state hash differs after snapshot/restore at input {at}"
    );
    assert_eq!(
        whole.final_version, resumed.final_version,
        "I-7 violated: state_version differs after snapshot/restore at input {at}"
    );
}

/// Resume a scenario from a restored state at `from`, reusing the original
/// indices so the RNG streams match the uninterrupted run.
fn continue_from<R: GameRules>(
    scenario: &Scenario<R>,
    mut state: R::State,
    from: usize,
    mut version: StateVersion,
) -> RunTrace {
    let mut checkpoints = Vec::new();
    let mut rejections = Vec::new();

    for (i, input) in scenario.inputs.iter().enumerate().skip(from) {
        // The original index, not a fresh count. Re-deriving from position in the
        // resumed run would give a different RNG stream and break replay.
        let index = InputIndex(i as u64 + 1);
        let mut rng = tabula_core::DetRng::for_input(&scenario.seed, index);
        let mut ctx = Ctx {
            now: LogicalTime(index.0 * 1_000),
            index,
            rng: &mut rng,
            budget: Budget {
                max_apply_micros: u32::MAX,
                max_events_per_input: u16::MAX,
            },
        };

        match R::apply(&mut state, input.clone(), &mut ctx) {
            Ok(_) => version = StateVersion(version.0 + 1),
            Err(e) => rejections.push((index.0, e.code)),
        }
        checkpoints.push((index.0, R::state_hash(&state)));
    }

    RunTrace {
        final_hash: R::state_hash(&state),
        final_state: encode(&state),
        final_version: version,
        checkpoints,
        events_encoded: Vec::new(),
        effects_encoded: Vec::new(),
        rejections,
    }
}

/// Every rejected input must leave the state **byte-identical**. (contract R2)
///
/// Compares the canonical encoding, not just `state_hash`: a game that overrides
/// `state_hash` with an incremental structure could otherwise hide a mutation the
/// incremental hash does not cover.
///
/// # Panics
/// Naming the input that mutated on rejection. That is a correctness bug of the
/// highest severity — a rejection is supposed to be a no-op, and a half-applied
/// one corrupts the match invisibly until a replay diverges.
pub fn assert_transactional_on_error<R: GameRules>(scenario: &Scenario<R>) {
    let mut rng = tabula_core::DetRng::for_input(&scenario.seed, InputIndex(0));
    let mut ctx = Ctx {
        now: LogicalTime::ZERO,
        index: InputIndex(0),
        rng: &mut rng,
        budget: Budget {
            max_apply_micros: u32::MAX,
            max_events_per_input: u16::MAX,
        },
    };
    let Ok(init) = R::create(&scenario.config, &scenario.roster, &mut ctx) else {
        return;
    };
    let mut state = init.state;
    let mut version = StateVersion(0);

    for (i, input) in scenario.inputs.iter().enumerate() {
        let index = InputIndex(i as u64 + 1);
        let before = encode(&state);
        let hash_before = R::state_hash(&state);

        let mut rng = tabula_core::DetRng::for_input(&scenario.seed, index);
        let mut ctx = Ctx {
            now: LogicalTime(index.0 * 1_000),
            index,
            rng: &mut rng,
            budget: Budget {
                max_apply_micros: u32::MAX,
                max_events_per_input: u16::MAX,
            },
        };

        let version_before = version;
        match R::apply(&mut state, input.clone(), &mut ctx) {
            Ok(_) => version = StateVersion(version.0 + 1),
            Err(code) => {
                let after = encode(&state);
                assert_eq!(
                    before,
                    after,
                    "R2 violated: input {} was rejected with {:?} but MUTATED the state.\n  \
                     hash before: {:02x?}\n  hash after:  {:02x?}\n\
                     Fix by validating fully BEFORE the first assignment to state \
                     (doc 02 §3.2).",
                    index.0,
                    code.code,
                    hash_before.0,
                    R::state_hash(&state).0
                );
                assert_eq!(
                    version, version_before,
                    "I-7 violated: state_version advanced on input {} which was rejected",
                    index.0
                );
            }
        }
    }
}

/// `state_version` +1 per accepted input, unchanged on rejection. (I-7)
///
/// # Panics
/// If the accepted-input count and the final version disagree.
pub fn assert_version_monotonic<R: GameRules>(scenario: &Scenario<R>) {
    let Ok(trace) = run::<R>(scenario) else {
        return;
    };
    let accepted = scenario.inputs.len() - trace.rejections.len();

    assert_eq!(
        trace.final_version.0, accepted as u64,
        "I-7 violated: {} inputs were accepted but state_version is {}. It must \
         advance by exactly 1 per accepted input and never otherwise.",
        accepted, trace.final_version.0
    );
}

/// `state` → canonical bytes → `state` preserves semantics and hash. (I-8)
///
/// This is the path production takes into `match_snapshots.payload` and out of a
/// `.tbr` file, so a failure here means snapshots and replays are unsound.
///
/// # Panics
/// If the round-trip loses information or changes the hash.
pub fn assert_state_roundtrip<R: GameRules>(scenario: &Scenario<R>) {
    let Ok(trace) = run::<R>(scenario) else {
        return;
    };

    let restored: R::State = canonical_decode(&trace.final_state)
        .expect("I-8 violated: canonical state could not be decoded back");
    let reencoded = encode(&restored);

    assert_eq!(
        trace.final_state, reencoded,
        "I-8 violated: state -> encode -> decode -> encode is not a fixed point. \
         Some field does not survive the canonical encoding."
    );
    assert_eq!(
        trace.final_hash,
        R::state_hash(&restored),
        "I-8 violated: the restored state hashes differently from the original"
    );
}

/// A rejected input cannot disturb any later input's randomness. (contract R8)
///
/// Applies a rejected input, then a probe input, and compares against applying
/// the probe alone from the untouched state. Both branches use the probe's own
/// index, so this asserts what the contract actually promises: the RNG stream is
/// a pure function of `(seed, index)` and no amount of drawing during a rejected
/// apply can shift it. (ADR-026 §5)
///
/// # Panics
/// If a rejection changed what a later input does.
pub fn assert_rejection_does_not_disturb_rng<R: GameRules>(
    scenario: &Scenario<R>,
    rejected: &Input<R::Command>,
    probe: &Input<R::Command>,
) {
    let Ok(trace) = run::<R>(scenario) else {
        return;
    };
    let Ok(base) = canonical_decode::<R::State>(&trace.final_state) else {
        return;
    };
    let probe_index = InputIndex(scenario.inputs.len() as u64 + 2);

    // Branch A: rejected input first (at its own index), then the probe.
    let mut with_rejection = base.clone();
    let reject_index = InputIndex(scenario.inputs.len() as u64 + 1);
    let mut rng = tabula_core::DetRng::for_input(&scenario.seed, reject_index);
    let outcome = R::apply(
        &mut with_rejection,
        rejected.clone(),
        &mut Ctx {
            now: LogicalTime(reject_index.0 * 1_000),
            index: reject_index,
            rng: &mut rng,
            budget: Budget {
                max_apply_micros: u32::MAX,
                max_events_per_input: u16::MAX,
            },
        },
    );
    assert!(
        outcome.is_err(),
        "test setup: the `rejected` input was accepted, so this asserts nothing"
    );
    let a = apply_probe::<R>(&mut with_rejection, probe, probe_index, &scenario.seed);

    // Branch B: the probe alone, from the untouched state.
    let mut clean = base;
    let b = apply_probe::<R>(&mut clean, probe, probe_index, &scenario.seed);

    assert_eq!(
        a, b,
        "R8 violated: a rejected input changed what the NEXT input did. The RNG \
         stream must be a pure function of (seed, index); a rejection must consume \
         nothing observable."
    );
    assert_eq!(
        encode(&with_rejection),
        encode(&clean),
        "R2/R8 violated: state after (rejection + probe) differs from (probe alone)"
    );
}

fn apply_probe<R: GameRules>(
    state: &mut R::State,
    probe: &Input<R::Command>,
    index: InputIndex,
    seed: &MatchSeed,
) -> Vec<Vec<u8>> {
    let mut rng = tabula_core::DetRng::for_input(seed, index);
    let mut ctx = Ctx {
        now: LogicalTime(index.0 * 1_000),
        index,
        rng: &mut rng,
        budget: Budget {
            max_apply_micros: u32::MAX,
            max_events_per_input: u16::MAX,
        },
    };
    match R::apply(state, probe.clone(), &mut ctx) {
        Ok(outcome) => outcome.events.iter().map(encode).collect(),
        Err(e) => vec![encode(&e.code)],
    }
}
