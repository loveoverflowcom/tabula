//! Durable, canonical, server-only Werewolf history evidence (I-8/I-16).
//!
//! Tests only read the corpus. An explicit `write_replays` example is the
//! intentional update path; fixtures always reconstruct from create and seed,
//! never a hand-edited `RawState` or an initial snapshot.

use std::{
    fs,
    path::{Path, PathBuf},
};

use tabula_core::{
    canonical_decode, DetRng, InputIndex, LogicalTime, OutcomeKind, RulesVersion, SeatId,
};
use tabula_game_api::{Budget, Ctx, Effect, GameRules, Input};
use tabula_game_werewolf::{
    Command, Config, Event, NightChoice, Phase, Role, State, WerewolfModule, WerewolfRules,
};
use tabula_testkit::{
    ReplayDraft, ReplayError, ReplayIdentity, ReplayKind, ReplayRunner, ReplayVerdict,
    ValidatedReplay,
};

const GOLDENS: [(&str, &str, usize, u64); 3] = [
    (
        "normal.tbr",
        "cd97e8a5d0e24207e0d31a03c5716d16d24b316e4edaa4cb453ed88d8e7cddf2",
        20,
        86,
    ),
    (
        "ties.tbr",
        "dcd29b76176c4e617d9e8a2e6a6077d91783a387330e8448929d1dfc90931061",
        12,
        27,
    ),
    (
        "timeout.tbr",
        "e4716b6d3c5f3072eb0ee8b83f5e3b291e2407549b8120582804249470d394eb",
        6,
        15,
    ),
];

fn replay_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/replays")
        .join(name)
}

#[test]
fn committed_complete_matches_have_exact_identity_checkpoints_and_pinned_final_hashes() {
    assert_eq!(WerewolfRules::RULES_VERSION, RulesVersion(2));
    for (name, final_hash, seats, inputs) in GOLDENS {
        let replay = ValidatedReplay::read(&replay_path(name)).expect("committed golden decodes");
        let header = replay.header();
        assert_eq!(header.game_id.as_str(), "com.tabula.werewolf");
        assert_eq!(header.kind, ReplayKind::Canonical);
        assert_eq!(header.rules_version, RulesVersion(2));
        assert_eq!(
            header.rules_hash,
            WerewolfRules::RULES_HASH,
            "{name}: stale source identity"
        );
        assert!(
            header.seed.is_some(),
            "canonical goldens reconstruct from the recorded seed"
        );
        assert!(
            header.initial_snapshot.is_none(),
            "goldens must start through create"
        );
        assert_eq!(header.roster.len(), seats);
        assert_eq!(
            replay.frames().len() as u64,
            inputs,
            "{name}: incomplete or changed script"
        );
        assert!(replay
            .frames()
            .iter()
            .all(|frame| frame.checkpoint.is_some()));
        assert_eq!(
            header.duration_ms,
            replay.frames().last().unwrap().logical_time.0
        );

        let mut runner = ReplayRunner::<WerewolfRules>::open(
            &replay_path(name),
            ReplayIdentity::from_module::<WerewolfModule>(),
        )
        .expect("typed golden runner opens");
        // is_verified alone permits CompatibleVersion and would miss a stale
        // golden header. Exact is a separate, mandatory historical claim.
        assert_eq!(runner.check(), ReplayVerdict::Exact, "{name}");
        let report = runner
            .verify()
            .expect("every accepted frame must apply successfully");
        assert_eq!(report.verdict(), &ReplayVerdict::Exact);
        assert!(report.is_verified(), "{name}: {report:?}");
        assert_eq!(report.inputs_replayed(), inputs);
        assert_eq!(report.checkpoints_checked(), inputs);
        assert!(report.final_hash_checked());
        assert!(report.outcome_checked());
        assert!(
            report.expected_outcome().is_some(),
            "golden must pin a terminal outcome"
        );
        assert_eq!(report.actual_outcome(), report.expected_outcome());
        assert_eq!(
            hex32(report.actual_final_state_hash().0),
            final_hash,
            "{name}"
        );
    }
}

#[test]
fn normal_golden_covers_all_roles_and_resolves_a_village_win_with_hunter_retaliation() {
    let (state, events) = execute("normal.tbr", |_, _| {});
    assert_eq!(state.phase(), Phase::Ended);
    assert_eq!(state.round(), 3);
    assert_eq!(state.alive().len(), 14);
    assert!(state
        .alive()
        .iter()
        .all(|seat| state.roles()[seat] != Role::Werewolf));
    assert!(state.hunter_fired());
    let outcome = state.outcome().unwrap();
    assert_eq!(outcome.kind(), OutcomeKind::Decisive);
    assert_eq!(outcome.summary(), "werewolf.village_wins");
    assert_eq!(outcome.standings().len(), 20);
    for standing in outcome.standings() {
        // Dead villagers, including the Hunter, share their faction's win.
        assert_eq!(
            standing.rank,
            u8::from(state.roles()[&standing.seat].is_wolf())
        );
    }
    for role in Role::ALL {
        assert!(state.roles().values().any(|assigned| *assigned == role));
    }
    for expected in ["wolf", "seer", "doctor", "heal", "poison", "hunter", "pass"] {
        assert!(
            events.iter().any(|event| {
                let Event::NightActionSubmitted { choice, .. } = event else {
                    return false;
                };
                matches!(
                    (expected, choice),
                    ("wolf", NightChoice::WolfTarget(Some(_)))
                        | ("seer", NightChoice::Investigate(_))
                        | ("doctor", NightChoice::Protect(Some(_)))
                        | ("heal", NightChoice::WitchHeal(Some(_)))
                        | ("poison", NightChoice::WitchPoison(Some(_)))
                        | ("hunter", NightChoice::HunterMark(Some(_)))
                        | ("pass", NightChoice::Pass)
                )
            }),
            "normal golden lacks {expected} choice"
        );
    }
    assert!(events.iter().any(|event| matches!(
        event,
        Event::HunterTriggered {
            target: Some(_),
            ..
        }
    )));
    assert!(events
        .iter()
        .any(|event| matches!(event, Event::SeerReport { .. })));
}

#[test]
fn tied_attack_and_six_six_vote_preserve_all_living_seats_before_the_cap_draw() {
    let (state, events) = execute("ties.tbr", |state, input| {
        if matches!(input, Input::Timer { .. }) {
            assert_eq!(
                state.alive().len(),
                12,
                "both ties and later defaults must leave every seat alive"
            );
        }
    });
    assert_eq!(state.round(), 2);
    assert_eq!(state.outcome().unwrap().kind(), OutcomeKind::Draw);
    assert_eq!(state.outcome().unwrap().summary(), "werewolf.stalemate");
    assert!(events.iter().any(|event| matches!(
        event,
        Event::NightResolved {
            round: 1,
            attacked: None,
            ..
        }
    )));
    let first_vote = events
        .iter()
        .find_map(|event| match event {
            Event::VoteResolved {
                round: 1,
                tally,
                eliminated,
            } => Some((tally, eliminated)),
            _ => None,
        })
        .expect("first round's vote resolution is recorded");
    assert_eq!(first_vote.0.len(), 2);
    assert_eq!(first_vote.0[&SeatId(0)], 6);
    assert_eq!(first_vote.0[&SeatId(6)], 6);
    assert_eq!(*first_vote.1, None);
    assert!(events
        .iter()
        .any(|event| matches!(event, Event::BallotChanged { ballot: None, .. })));
    let replay = ValidatedReplay::read(&replay_path("ties.tbr")).unwrap();
    let wolf_targets: Vec<_> = replay
        .frames()
        .iter()
        .filter_map(
            |frame| match canonical_decode::<Input<Command>>(&frame.input).unwrap() {
                Input::Player {
                    command: Command::Night(NightChoice::WolfTarget(Some(target))),
                    ..
                } => Some(target),
                _ => None,
            },
        )
        .collect();
    assert_eq!(wolf_targets.len(), 2);
    assert_ne!(
        wolf_targets[0], wolf_targets[1],
        "a night tie needs distinct equal candidates"
    );
}

#[test]
fn all_timeout_golden_expires_every_phase_at_its_exact_deadline_and_draws_at_dusk_cap() {
    let replay = ValidatedReplay::read(&replay_path("timeout.tbr")).unwrap();
    let config: Config = canonical_decode(&replay.header().config).unwrap();
    assert_eq!(config.max_rounds.get(), 3);
    let (state, events) = execute("timeout.tbr", |state, input| {
        assert!(
            matches!(input, Input::Timer { .. }),
            "timeout golden must not depend on player/admin actions"
        );
        assert_eq!(state.alive().len(), 6);
    });
    assert_eq!(state.phase(), Phase::Ended);
    assert_eq!(state.round(), config.max_rounds.get());
    assert_eq!(state.outcome().unwrap().kind(), OutcomeKind::Draw);
    assert_eq!(state.outcome().unwrap().summary(), "werewolf.stalemate");
    assert!(state
        .outcome()
        .unwrap()
        .standings()
        .iter()
        .all(|standing| standing.rank == 0));
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, Event::NightResolved { .. }))
            .count(),
        3
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, Event::VoteResolved { .. }))
            .count(),
        3
    );
    assert!(!events.iter().any(|event| matches!(
        event,
        Event::NightActionSubmitted { .. }
            | Event::BallotChanged { .. }
            | Event::DeathRevealed { .. }
    )));
}

#[test]
fn stale_source_hash_is_compatible_only_and_old_rules_version_is_explicitly_unsupported() {
    let replay = ValidatedReplay::read(&replay_path("normal.tbr")).unwrap();
    let mut stale = ReplayDraft {
        header: replay.header().clone(),
        frames: replay.frames().to_vec(),
        final_state_hash: replay.final_state_hash(),
    };
    stale.header.rules_hash[0] ^= 1;
    let runner = ReplayRunner::<WerewolfRules>::from_bytes(
        &stale.to_bytes().unwrap(),
        ReplayIdentity::from_module::<WerewolfModule>(),
    )
    .unwrap();
    assert_eq!(runner.check(), ReplayVerdict::CompatibleVersion);
    stale.header.rules_version = RulesVersion(1);
    let error = ReplayRunner::<WerewolfRules>::from_bytes(
        &stale.to_bytes().unwrap(),
        ReplayIdentity::from_module::<WerewolfModule>(),
    )
    .unwrap_err();
    assert!(matches!(error, ReplayError::Unreplayable(_)));
}

/// Replay through the ordinary authority path while retaining semantic events
/// independently of `ReplayRunner`'s checkpoint/outcome verification.
fn execute(name: &str, mut inspect: impl FnMut(&State, &Input<Command>)) -> (State, Vec<Event>) {
    let replay = ValidatedReplay::read(&replay_path(name)).unwrap();
    let config: Config = canonical_decode(&replay.header().config).unwrap();
    let seed = replay.header().seed.as_ref().unwrap();
    let mut rng = DetRng::for_input(seed, InputIndex(0));
    let init = WerewolfRules::create(
        &config,
        &replay.header().roster,
        &mut context(&mut rng, InputIndex(0), LogicalTime::ZERO),
    )
    .unwrap();
    let mut state = init.state;
    let mut events = init.events.to_vec();
    let mut terminal = None;
    for frame in replay.frames() {
        assert!(terminal.is_none(), "canonical inputs must stop at EndMatch");
        let input: Input<Command> = canonical_decode(&frame.input).unwrap();
        match &input {
            Input::Timer { timer } => {
                assert_eq!(
                    *timer,
                    state.current_timer(),
                    "golden expires its current timer"
                );
                assert_eq!(
                    frame.logical_time,
                    state.phase_ends_at(),
                    "timer fires at the exact recorded deadline"
                );
            }
            Input::Player { .. } => assert!(frame.logical_time < state.phase_ends_at()),
            _ => panic!("goldens may not shortcut outcomes with admin or lifecycle input"),
        }
        let mut rng = DetRng::for_input(seed, frame.input_index);
        let output = WerewolfRules::apply(
            &mut state,
            input.clone(),
            &mut context(&mut rng, frame.input_index, frame.logical_time),
        )
        .unwrap();
        assert_eq!(
            Some(WerewolfRules::state_hash(&state)),
            frame.checkpoint,
            "{name}: input {}",
            frame.input_index.0
        );
        for effect in output.effects {
            if let Effect::EndMatch { outcome } = effect {
                assert!(terminal.replace(outcome).is_none());
            }
        }
        events.extend(output.events);
        inspect(&state, &input);
    }
    assert_eq!(terminal.as_ref(), replay.header().outcome.as_ref());
    assert!(terminal.is_some());
    assert_eq!(state.outcome(), terminal.as_ref());
    (state, events)
}

fn context(rng: &mut DetRng, index: InputIndex, now: LogicalTime) -> Ctx<'_> {
    Ctx {
        now,
        index,
        rng,
        budget: Budget {
            max_apply_micros: u32::MAX,
            max_events_per_input: u16::MAX,
        },
    }
}

#[test]
fn rules_hash_matches_independently_sorted_recursive_source_preimage() {
    let sources = independent_rules_sources();
    assert!(!sources.is_empty());
    assert!(sources.windows(2).all(|files| files[0].0 < files[1].0));
    assert_eq!(WerewolfRules::RULES_HASH, independent_rules_hash(&sources));
    // The path and bytes must each affect the identity, not only the count.
    let mut changed = sources.clone();
    changed.last_mut().unwrap().1.push(b'\n');
    assert_ne!(
        independent_rules_hash(&sources),
        independent_rules_hash(&changed)
    );
    changed = sources.clone();
    changed.last_mut().unwrap().0 = PathBuf::from("renamed.rs");
    assert_ne!(
        independent_rules_hash(&sources),
        independent_rules_hash(&changed)
    );
}

#[test]
fn canonical_rules_source_cannot_reach_unhashed_root_or_presentation_sources() {
    for (relative, bytes) in independent_rules_sources() {
        let source = std::str::from_utf8(&bytes).unwrap();
        for (number, line) in source.lines().enumerate() {
            let code = line.split("//").next().unwrap_or("");
            for forbidden in ["crate::", "super::super::", "#[path", "include!"] {
                assert!(
                    !code.contains(forbidden),
                    "{}:{} reaches {forbidden}",
                    relative.display(),
                    number + 1
                );
            }
        }
    }
}

fn independent_rules_sources() -> Vec<(PathBuf, Vec<u8>)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/rules");
    let mut paths = Vec::new();
    collect_sources(&root, &root, &mut paths);
    paths.sort();
    paths
        .into_iter()
        .map(|path| {
            let bytes = fs::read(root.join(&path)).unwrap();
            (path, bytes)
        })
        .collect()
}

fn collect_sources(root: &Path, directory: &Path, paths: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect_sources(root, &path, paths);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            paths.push(path.strip_prefix(root).unwrap().to_owned());
        }
    }
}

fn independent_rules_hash(sources: &[(PathBuf, Vec<u8>)]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"tabula.rules.source.v2");
    hasher.update(&WerewolfRules::RULES_VERSION.0.to_le_bytes());
    for (relative, bytes) in sources {
        let relative = relative.to_string_lossy().replace('\\', "/");
        hasher.update(&(relative.len() as u64).to_le_bytes());
        hasher.update(relative.as_bytes());
        hasher.update(&(bytes.len() as u64).to_le_bytes());
        hasher.update(bytes);
    }
    *hasher.finalize().as_bytes()
}

fn hex32(bytes: [u8; 32]) -> String {
    use std::fmt::Write;
    bytes
        .iter()
        .fold(String::with_capacity(64), |mut output, byte| {
            write!(output, "{byte:02x}").unwrap();
            output
        })
}
