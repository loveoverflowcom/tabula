//! SDK conformance over real private actions, fixed timers and replaceable ballots.
//! Game-specific security assertions live in `security.rs` (I-5/I-6).

use tabula_core::{
    DetRng, InputIndex, LogicalTime, MatchSeed, Millis, Occupant, SeatEntry, SeatId, SeatRoster,
    UserId,
};
use tabula_game_api::{Budget, Ctx, GameRules, Input};
use tabula_game_werewolf::{
    Ballot, Command, Config, MaxRounds, NightChoice, Phase, PhaseDuration, PhaseDurations, Role,
    State, WerewolfModule, WerewolfRules,
};
use tabula_testkit::{
    GameTestFixture, InvalidCommandScenario, RandomnessScenario, TerminalScenario,
};

const SEED: [u8; 32] = [42; 32];
const SEATS: u8 = 12;

fn roster() -> SeatRoster {
    SeatRoster::new(
        (0..SEATS)
            .map(|index| SeatEntry {
                seat: SeatId(index),
                occupant: Occupant::Human(UserId(u128::from(index) + 1)),
                team: None,
            })
            .collect(),
    )
    .expect("fixture seats are unique")
}

fn config() -> Config {
    let short = PhaseDuration::from_millis(Millis(1_000)).unwrap();
    let window = PhaseDuration::from_millis(Millis(4_000)).unwrap();
    Config {
        phase_durations: PhaseDurations {
            night: window,
            dawn: short,
            day: short,
            vote: window,
            dusk: short,
        },
        max_rounds: MaxRounds::new(1).unwrap(),
        ..Config::default()
    }
}

fn initial() -> State {
    let seed = MatchSeed::from_bytes(SEED);
    let mut rng = DetRng::for_input(&seed, InputIndex(0));
    let mut ctx = Ctx {
        now: LogicalTime::ZERO,
        index: InputIndex(0),
        rng: &mut rng,
        budget: Budget::default(),
    };
    WerewolfRules::create(&config(), &roster(), &mut ctx)
        .expect("fixture creation succeeds")
        .state
}

fn seat_with(state: &State, role: Role) -> SeatId {
    *state
        .roles()
        .iter()
        .find(|(_, assigned)| **assigned == role)
        .expect("ClassicV1 at twelve seats contains this role")
        .0
}

fn player(seat: SeatId, command: Command) -> Input<Command> {
    Input::Player { seat, command }
}

fn apply_indexed(state: &mut State, input: Input<Command>, index: u64) {
    let seed = MatchSeed::from_bytes(SEED);
    let index = InputIndex(index);
    let mut rng = DetRng::for_input(&seed, index);
    let mut ctx = Ctx {
        now: LogicalTime(index.0 * 1_000),
        index,
        rng: &mut rng,
        budget: Budget::default(),
    };
    WerewolfRules::apply(state, input, &mut ctx).expect("fixture input is legal");
}

/// Build timers from the authoritative state, keeping the testkit's logical
/// time schedule (one second per input) and asserting every fixture input.
fn script(terminal: bool) -> Vec<Input<Command>> {
    let mut state = initial();
    let seer = seat_with(&state, Role::Seer);
    let doctor = seat_with(&state, Role::Doctor);
    let wolf = seat_with(&state, Role::Werewolf);
    let mut inputs = vec![
        player(seer, Command::Night(NightChoice::Investigate(wolf))),
        player(doctor, Command::Night(NightChoice::Protect(Some(doctor)))),
        player(wolf, Command::Night(NightChoice::Pass)),
    ];
    for (index, input) in inputs.iter().enumerate() {
        apply_indexed(&mut state, input.clone(), index as u64 + 1);
    }
    for expected in [Phase::Night, Phase::Dawn, Phase::Day] {
        assert_eq!(state.phase(), expected);
        let input = Input::Timer {
            timer: state.current_timer(),
        };
        apply_indexed(&mut state, input.clone(), inputs.len() as u64 + 1);
        inputs.push(input);
    }
    assert_eq!(state.phase(), Phase::Vote);
    for command in [
        Command::Vote(Ballot::Abstain),
        Command::Vote(Ballot::Target(SeatId(1))),
        Command::Unvote,
    ] {
        let input = player(SeatId(0), command);
        apply_indexed(&mut state, input.clone(), inputs.len() as u64 + 1);
        inputs.push(input);
    }
    assert_eq!(state.phase(), Phase::Vote);
    assert!(
        state.votes().is_empty(),
        "Unvote clears the replacement ballot"
    );
    if terminal {
        for expected in [Phase::Vote, Phase::Dusk] {
            assert_eq!(state.phase(), expected);
            let input = Input::Timer {
                timer: state.current_timer(),
            };
            apply_indexed(&mut state, input.clone(), inputs.len() as u64 + 1);
            inputs.push(input);
        }
        assert_eq!(state.phase(), Phase::Ended);
    }
    inputs
}

struct WerewolfFixture;

impl GameTestFixture for WerewolfFixture {
    type Module = WerewolfModule;

    fn config() -> Config {
        config()
    }

    fn roster() -> SeatRoster {
        roster()
    }

    fn seed() -> MatchSeed {
        MatchSeed::from_bytes(SEED)
    }

    fn deterministic_script() -> Vec<Input<Command>> {
        script(false)
    }

    fn invalid_command() -> Option<InvalidCommandScenario<Command>> {
        let state = initial();
        Some(InvalidCommandScenario {
            setup: Vec::new(),
            invalid: player(SeatId(0), Command::Vote(Ballot::Abstain)),
            probe: player(
                seat_with(&state, Role::Seer),
                Command::Night(NightChoice::Investigate(seat_with(&state, Role::Werewolf))),
            ),
        })
    }

    fn terminal() -> Option<TerminalScenario<Command>> {
        Some(TerminalScenario {
            script: script(true),
            post_terminal: player(SeatId(0), Command::Vote(Ballot::Abstain)),
        })
    }

    fn randomness() -> Option<RandomnessScenario> {
        Some(RandomnessScenario {
            alternate_seed: MatchSeed::from_bytes([7; 32]),
        })
    }
}

tabula_testkit::conformance!(WerewolfFixture);
