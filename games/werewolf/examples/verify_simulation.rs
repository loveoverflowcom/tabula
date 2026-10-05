//! Explicit local verification campaign, never a game host or production bot.
use tabula_core::{BotLevel, Occupant, SeatEntry, SeatId, SeatRoster};
use tabula_game_werewolf::{simulation::SimulationModule, Config, MaxRounds};
use tabula_testkit::selfplay::{run, SelfPlayConfig, SelfPlaySetup};
fn main() {
    let count: u32 = std::env::args()
        .nth(1)
        .map_or(Ok(1_000), |value| value.parse())
        .expect("numeric match count");
    let seats: u8 = std::env::args()
        .nth(2)
        .map_or(Ok(12), |value| value.parse())
        .expect("numeric seat count");
    let setup = SelfPlaySetup {
        config: Config {
            max_rounds: MaxRounds::new(3).unwrap(),
            ..Config::default()
        },
        roster: SeatRoster::new(
            (0..seats)
                .map(|seat| SeatEntry {
                    seat: SeatId(seat),
                    occupant: Occupant::Bot {
                        level: BotLevel::Trivial,
                    },
                    team: None,
                })
                .collect(),
        )
        .unwrap(),
    };
    let cfg = SelfPlayConfig {
        matches: count,
        base_seed: [64; 32],
        hostile_fraction: 0.05,
        max_inputs: 1_000,
        check_projections: true,
        start_match_index: 0,
    };
    let report = run::<SimulationModule>(&setup, &cfg).expect("valid simulation setup");
    println!("seats={seats} max_rounds=3 base_seed=[64;32] hostile_fraction=0.05 check_projections=true matches={} terminated={} inputs={} failures={} determinism_failures={} transactional_failures={} max_input_failures={} p99_apply_micros={}", report.matches_run, report.terminated, report.inputs_total, report.failures.len(), report.determinism_failures, report.transactional_failures, report.max_input_failures,report.p99_apply_micros);
    for failure in &report.failures {
        eprintln!("{failure:?}");
    }
    assert!(report.is_success());
}
