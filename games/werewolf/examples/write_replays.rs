//! Explicit canonical golden-corpus writer (I-8/I-16, doc 05 §8).
//!
//! Run only after an intentional rules/corpus update:
//! `cargo run -p tabula-game-werewolf --example write_replays -- DESTINATION`.
//! This audit-only example never runs from tests and never serves its seed or
//! canonical inputs to a client. Every match starts with `GameRules::create`.

use std::{env, fs, path::Path};

use tabula_core::{
    canonical_encode, BotLevel, DetRng, InputIndex, LogicalTime, MatchId, MatchSeed, Millis,
    Occupant, SeatEntry, SeatId, SeatRoster,
};
use tabula_game_api::{Budget, Ctx, Effect, GameModule, GameRules, Input};
use tabula_game_werewolf::{
    Ballot, Command, Config, MaxRounds, NightChoice, Phase, PhaseDuration, PhaseDurations, Role,
    State, WerewolfModule, WerewolfRules,
};
use tabula_testkit::{
    ReplayDraft, ReplayFrame, ReplayHeader, ReplayIdentity, ReplayKind, ReplayRunner, ReplayVerdict,
};

fn main() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let destination = args.next().ok_or_else(|| {
        "usage: cargo run -p tabula-game-werewolf --example write_replays -- DESTINATION".to_owned()
    })?;
    if args.next().is_some() {
        return Err("only one explicit destination directory is supported".to_owned());
    }
    let directory = Path::new(&destination);
    fs::create_dir_all(directory).map_err(|error| error.to_string())?;
    normal()?.write(&directory.join("normal.tbr"), 201)?;
    ties()?.write(&directory.join("ties.tbr"), 202)?;
    timeout()?.write(&directory.join("timeout.tbr"), 203)
}

struct Script {
    config: Config,
    roster: SeatRoster,
    seed: MatchSeed,
    state: State,
    now: LogicalTime,
    frames: Vec<ReplayFrame>,
    terminal: Option<tabula_core::MatchOutcome>,
}

impl Script {
    fn new(seats: u8, cap: u32, seed_byte: u8) -> Result<Self, String> {
        let duration = PhaseDuration::from_millis(Millis(1_000)).map_err(|e| e.to_string())?;
        let config = Config {
            phase_durations: PhaseDurations {
                night: duration,
                dawn: duration,
                day: duration,
                vote: duration,
                dusk: duration,
            },
            max_rounds: MaxRounds::new(cap).map_err(|e| e.to_string())?,
            ..Config::default()
        };
        let roster = SeatRoster::new(
            (0..seats)
                .map(|index| SeatEntry {
                    seat: SeatId(index),
                    // Fixed pseudonymous occupants for audit only; these do
                    // not install a game bot or permit runtime substitution.
                    occupant: Occupant::Bot {
                        level: BotLevel::Trivial,
                    },
                    team: None,
                })
                .collect(),
        )
        .map_err(|e| e.to_string())?;
        WerewolfModule::validate_config(&config, &roster).map_err(|e| e.to_string())?;
        let seed = MatchSeed::from_bytes([seed_byte; 32]);
        let mut rng = DetRng::for_input(&seed, InputIndex(0));
        let init = WerewolfRules::create(&config, &roster, &mut context(&mut rng, 0, 0))
            .map_err(|e| format!("create: {e:?}"))?;
        Ok(Self {
            config,
            roster,
            seed,
            state: init.state,
            now: LogicalTime::ZERO,
            frames: Vec::new(),
            terminal: None,
        })
    }

    fn submit(&mut self, seat: SeatId, command: Command) -> Result<(), String> {
        self.apply(Input::Player { seat, command }, LogicalTime(self.now.0 + 1))
    }

    fn expire(&mut self) -> Result<(), String> {
        self.apply(
            Input::Timer {
                timer: self.state.current_timer(),
            },
            self.state.phase_ends_at(),
        )
    }

    fn apply(&mut self, input: Input<Command>, now: LogicalTime) -> Result<(), String> {
        if self.terminal.is_some() {
            return Err("script attempted an input after EndMatch".to_owned());
        }
        let index = self.frames.len() as u64 + 1;
        let mut rng = DetRng::for_input(&self.seed, InputIndex(index));
        let encoded_input = canonical_encode(&input).map_err(|e| e.to_string())?;
        let output =
            WerewolfRules::apply(&mut self.state, input, &mut context(&mut rng, index, now.0))
                .map_err(|e| format!("input {index}: {e:?}"))?;
        for effect in output.effects {
            if let Effect::EndMatch { outcome } = effect {
                if self.terminal.replace(outcome).is_some() {
                    return Err("duplicate EndMatch effect".to_owned());
                }
            }
        }
        self.frames.push(ReplayFrame {
            input_index: InputIndex(index),
            logical_time: now,
            input: encoded_input,
            checkpoint: Some(WerewolfRules::state_hash(&self.state)),
        });
        self.now = now;
        Ok(())
    }

    fn seats_with_role(&self, role: Role) -> Vec<SeatId> {
        self.state
            .alive()
            .iter()
            .copied()
            .filter(|seat| self.state.roles()[seat] == role)
            .collect()
    }

    fn role(&self, role: Role) -> Result<SeatId, String> {
        self.seats_with_role(role)
            .first()
            .copied()
            .ok_or_else(|| format!("script requires living {role:?}"))
    }

    fn advance_to_vote(&mut self) -> Result<(), String> {
        for phase in [Phase::Night, Phase::Dawn, Phase::Day] {
            if self.state.phase() != phase {
                return Err(format!(
                    "expected {phase:?}, found {:?}",
                    self.state.phase()
                ));
            }
            self.expire()?;
        }
        Ok(())
    }

    fn village_vote(&mut self) -> Result<(), String> {
        let target = self.role(Role::Werewolf)?;
        let voters: Vec<_> = self.state.alive().iter().copied().collect();
        for seat in voters {
            self.submit(
                seat,
                Command::Vote(if seat == target {
                    Ballot::Abstain
                } else {
                    Ballot::Target(target)
                }),
            )?;
        }
        self.expire()
    }

    fn write(self, path: &Path, match_id: u64) -> Result<(), String> {
        if self.state.phase() != Phase::Ended || self.terminal.is_none() {
            return Err("goldens must contain a complete terminal match".to_owned());
        }
        if self.state.outcome() != self.terminal.as_ref() {
            return Err("state and EndMatch outcomes disagree".to_owned());
        }
        let final_state_hash = WerewolfRules::state_hash(&self.state);
        let draft = ReplayDraft {
            header: ReplayHeader {
                match_id: MatchId(u128::from(match_id)),
                game_id: WerewolfModule::metadata().id().clone(),
                game_version: WerewolfModule::metadata().version().clone(),
                rules_version: WerewolfRules::RULES_VERSION,
                rules_hash: WerewolfRules::RULES_HASH,
                config: canonical_encode(&self.config).map_err(|e| e.to_string())?,
                roster: self.roster,
                seed: Some(self.seed),
                initial_snapshot: None,
                started_at: 0,
                duration_ms: self.now.0,
                outcome: self.terminal,
                kind: ReplayKind::Canonical,
            },
            frames: self.frames,
            final_state_hash,
        };
        let bytes = draft.to_bytes().map_err(|e| e.to_string())?;
        let mut runner = ReplayRunner::<WerewolfRules>::from_bytes(
            &bytes,
            ReplayIdentity::from_module::<WerewolfModule>(),
        )
        .map_err(|e| e.to_string())?;
        if runner.check() != ReplayVerdict::Exact {
            return Err("generated replay lacks exact rules identity".to_owned());
        }
        let report = runner.verify().map_err(|e| e.to_string())?;
        if !report.is_verified() {
            return Err(format!("generated replay failed verification: {report:?}"));
        }
        fs::write(path, bytes).map_err(|e| e.to_string())?;
        println!(
            "{}: inputs={}, final hash={:02x?}, outcome={:?}",
            path.display(),
            report.inputs_replayed(),
            final_state_hash.0,
            report.actual_outcome()
        );
        Ok(())
    }
}

fn normal() -> Result<Script, String> {
    let mut script = Script::new(20, 5, 0x61)?;
    let village = script.role(Role::Villager)?;
    let doctor = script.role(Role::Doctor)?;
    let seer = script.role(Role::Seer)?;
    let witch = script.role(Role::Witch)?;
    let hunter = script.role(Role::Hunter)?;
    let wolves = script.seats_with_role(Role::Werewolf);

    // First Night exercises blind healing, protection and investigation.
    for &wolf in &wolves {
        script.submit(wolf, Command::Night(NightChoice::WolfTarget(Some(village))))?;
    }
    script.submit(doctor, Command::Night(NightChoice::Protect(Some(village))))?;
    script.submit(seer, Command::Night(NightChoice::Investigate(wolves[0])))?;
    script.submit(witch, Command::Night(NightChoice::WitchHeal(Some(village))))?;
    script.submit(
        hunter,
        Command::Night(NightChoice::HunterMark(Some(wolves[0]))),
    )?;
    script.advance_to_vote()?;
    script.village_vote()?;
    script.expire()?; // Dusk -> Night 2

    // The second batch kills Hunter and independently poisons one wolf. Hunter
    // retaliates against another wolf; all choices precede the same expiry.
    let wolves = script.seats_with_role(Role::Werewolf);
    for &wolf in &wolves {
        script.submit(wolf, Command::Night(NightChoice::WolfTarget(Some(hunter))))?;
    }
    script.submit(doctor, Command::Night(NightChoice::Protect(Some(doctor))))?;
    script.submit(seer, Command::Night(NightChoice::Investigate(wolves[2])))?;
    script.submit(
        witch,
        Command::Night(NightChoice::WitchPoison(Some(wolves[0]))),
    )?;
    script.submit(
        hunter,
        Command::Night(NightChoice::HunterMark(Some(wolves[1]))),
    )?;
    script.advance_to_vote()?;
    script.village_vote()?;
    script.expire()?; // Dusk -> Night 3

    let wolf = script.role(Role::Werewolf)?;
    script.submit(wolf, Command::Night(NightChoice::WolfTarget(Some(village))))?;
    script.submit(doctor, Command::Night(NightChoice::Protect(Some(village))))?;
    script.submit(seer, Command::Night(NightChoice::Investigate(wolf)))?;
    script.submit(witch, Command::Night(NightChoice::Pass))?;
    script.advance_to_vote()?;
    script.village_vote()?;
    Ok(script)
}

fn ties() -> Result<Script, String> {
    let mut script = Script::new(12, 2, 0x62)?;
    let wolves = script.seats_with_role(Role::Werewolf);
    let villagers = script.seats_with_role(Role::Villager);
    // Two equal attack candidates and a passing third wolf produce no attack.
    script.submit(
        wolves[0],
        Command::Night(NightChoice::WolfTarget(Some(villagers[0]))),
    )?;
    script.submit(
        wolves[1],
        Command::Night(NightChoice::WolfTarget(Some(villagers[1]))),
    )?;
    script.submit(wolves[2], Command::Night(NightChoice::Pass))?;
    script.advance_to_vote()?;

    // Replacement, explicit abstention and Unvote are accepted inputs. The
    // final ballots produce an exact 6/6 tie with no self-votes.
    script.submit(SeatId(0), Command::Vote(Ballot::Abstain))?;
    script.submit(SeatId(0), Command::Unvote)?;
    for index in 0..12 {
        let target = if index < 6 { SeatId(6) } else { SeatId(0) };
        script.submit(SeatId(index), Command::Vote(Ballot::Target(target)))?;
    }
    script.expire()?;
    // Finish with defaults through the configured Dusk cap, never ForceEnd.
    while script.state.phase().is_playing() {
        script.expire()?;
    }
    Ok(script)
}

fn timeout() -> Result<Script, String> {
    let mut script = Script::new(6, 3, 0x63)?;
    while script.state.phase().is_playing() {
        script.expire()?;
    }
    Ok(script)
}

fn context(rng: &mut DetRng, index: u64, now: u64) -> Ctx<'_> {
    Ctx {
        now: LogicalTime(now),
        index: InputIndex(index),
        rng,
        budget: Budget {
            max_apply_micros: u32::MAX,
            max_events_per_input: u16::MAX,
        },
    }
}
