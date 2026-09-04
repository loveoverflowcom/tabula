//! `cargo xtask selfplay <game>` — deterministic bot-vs-bot verification.

use std::{env, fmt::Write as _};

use tabula_core::{BotLevel, Occupant, SeatEntry, SeatId, SeatRoster};
use tabula_game_chess::{ChessModule, ClockConfig, ClockControl, Config as ChessConfig};
use tabula_game_tiles::{Config as TilesConfig, TilesModule};
use tabula_testkit::selfplay::{SelfPlayConfig, SelfPlayReport, SelfPlaySetup};

#[derive(Clone, Copy)]
enum ClockMode {
    Fischer,
    Bronstein,
    None,
}

impl ClockMode {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "fischer" => Ok(Self::Fischer),
            "bronstein" => Ok(Self::Bronstein),
            "none" => Ok(Self::None),
            _ => Err(format!(
                "invalid clock mode {value:?}; expected fischer, bronstein, or none"
            )),
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Fischer => "fischer",
            Self::Bronstein => "bronstein",
            Self::None => "none",
        }
    }
}

pub(crate) fn run() -> Result<(), String> {
    let mut args = env::args().skip(2);
    let game = args.next().ok_or_else(|| usage("a game is required"))?;
    let mut cfg = SelfPlayConfig::default();
    let mut clock = ClockMode::Fischer;
    let mut seats: u8 = 3;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--matches" => {
                cfg.matches = next_value(&mut args, "--matches")?
                    .parse()
                    .map_err(|_| "--matches must be an unsigned integer".to_owned())?;
            }
            "--seed" => cfg.base_seed = parse_seed(&next_value(&mut args, "--seed")?)?,
            "--match-index" => {
                cfg.start_match_index = next_value(&mut args, "--match-index")?
                    .parse()
                    .map_err(|_| "--match-index must be an unsigned integer".to_owned())?;
            }
            "--max-inputs" => {
                cfg.max_inputs = next_value(&mut args, "--max-inputs")?
                    .parse()
                    .map_err(|_| "--max-inputs must be an unsigned integer".to_owned())?;
            }
            "--clock" if game == "chess" => {
                clock = ClockMode::parse(&next_value(&mut args, "--clock")?)?;
            }
            "--seats" if game == "tiles" => {
                seats = next_value(&mut args, "--seats")?
                    .parse()
                    .map_err(|_| "--seats must be an unsigned integer".to_owned())?;
            }
            "--no-projection-check" => cfg.check_projections = false,
            _ => return Err(usage(&format!("unknown argument {arg:?}"))),
        }
    }

    let report = match game.as_str() {
        "chess" => run_chess(&cfg, clock)?,
        "tiles" => run_tiles(&cfg, seats)?,
        _ => return Err(usage(&format!("unsupported game {game:?}"))),
    };
    print_report(&game, &cfg, clock, &report);
    if report.is_success() {
        Ok(())
    } else {
        Err("one or more self-play matches failed".to_owned())
    }
}

fn run_chess(cfg: &SelfPlayConfig, clock: ClockMode) -> Result<SelfPlayReport, String> {
    let config = ChessConfig {
        clock: match clock {
            ClockMode::Fischer => Some(ClockConfig {
                initial: tabula_core::Millis(60_000),
                control: ClockControl::Fischer {
                    increment: tabula_core::Millis(1_000),
                },
            }),
            ClockMode::Bronstein => Some(ClockConfig {
                initial: tabula_core::Millis(60_000),
                control: ClockControl::Bronstein {
                    delay: tabula_core::Millis(1_000),
                },
            }),
            ClockMode::None => None,
        },
    };
    let setup = SelfPlaySetup::<tabula_game_chess::ChessRules> {
        config,
        roster: bot_roster(2),
    };
    tabula_testkit::selfplay::run::<ChessModule>(&setup, cfg).map_err(|error| error.to_string())
}

/// Tiles self-play. `--seats` matters here in a way it does not for the
/// two-seat games: turn order, follower supply, and majority ties all change
/// with the seat count, so the nightly campaign should sweep it.
fn run_tiles(cfg: &SelfPlayConfig, seats: u8) -> Result<SelfPlayReport, String> {
    if !(tabula_game_tiles::rules::MIN_SEATS..=tabula_game_tiles::rules::MAX_SEATS).contains(&seats)
    {
        return Err(format!(
            "--seats must be between {} and {}",
            tabula_game_tiles::rules::MIN_SEATS,
            tabula_game_tiles::rules::MAX_SEATS
        ));
    }
    let setup = SelfPlaySetup::<tabula_game_tiles::TilesRules> {
        config: TilesConfig {
            // No deadline: the bots always answer, so a deadline would only
            // ever fire as a side effect of the harness's own scheduling.
            turn_deadline_ms: 0,
        },
        roster: bot_roster(seats),
    };
    tabula_testkit::selfplay::run::<TilesModule>(&setup, cfg).map_err(|error| error.to_string())
}

fn bot_roster(count: u8) -> SeatRoster {
    let entries = (0..count)
        .map(|seat| SeatEntry {
            seat: SeatId(seat),
            occupant: Occupant::Bot {
                level: BotLevel::Trivial,
            },
            team: None,
        })
        .collect();
    SeatRoster::new(entries).expect("CLI bot roster uses distinct seats")
}

fn print_report(game: &str, cfg: &SelfPlayConfig, clock: ClockMode, report: &SelfPlayReport) {
    println!(
        "Game: {game}\nMatches: {} / {}\nInputs: {}\nTerminated: {}\nFailures: {}\nDeterminism: {}",
        report.matches_run,
        cfg.matches,
        report.inputs_total,
        report.terminated,
        report.failures.len(),
        if report.determinism_failures == 0 {
            "OK"
        } else {
            "FAIL"
        }
    );
    if let Some(failure) = report.failures.first() {
        let seed = format_seed(&failure.base_seed);
        let input_index = failure
            .input_index
            .map_or_else(|| "<none>".to_owned(), |index| index.to_string());
        println!(
            "FAILED\nmatch_index: {}\nbase_seed: {seed}\ninput_index: {input_index}\nkind: {}\nreason: {}\nreproduce:\ncargo xtask selfplay {game} --matches 1 --seed {seed} --match-index {}{}",
            failure.match_index,
            failure.kind,
            failure.reason,
            failure.match_index,
            if game == "chess" {
                format!(" --clock {}", clock.label())
            } else {
                String::new()
            },
        );
    }
}

fn next_value(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, String> {
    args.next()
        .ok_or_else(|| usage(&format!("{flag} requires a value")))
}

fn parse_seed(value: &str) -> Result<[u8; 32], String> {
    if let Ok(number) = value.parse::<u64>() {
        let mut seed = [0_u8; 32];
        seed[..8].copy_from_slice(&number.to_le_bytes());
        return Ok(seed);
    }
    if value.len() != 64 {
        return Err("--seed must be an unsigned integer or 64 hexadecimal characters".to_owned());
    }
    let raw = value.as_bytes();
    let mut seed = [0_u8; 32];
    for (index, byte) in seed.iter_mut().enumerate() {
        let high = hex_nibble(raw[index * 2]);
        let low = hex_nibble(raw[index * 2 + 1]);
        let (Some(high), Some(low)) = (high, low) else {
            return Err("--seed contains non-hexadecimal characters".to_owned());
        };
        *byte = (high << 4) | low;
    }
    Ok(seed)
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn format_seed(seed: &[u8; 32]) -> String {
    let mut output = String::with_capacity(64);
    for byte in seed {
        write!(output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

fn usage(reason: &str) -> String {
    format!(
        "{reason}\nusage: cargo xtask selfplay <chess|tiles> [--matches N] [--seed N|HEX] [--match-index N] [--max-inputs N] [--clock fischer|bronstein|none] [--seats N]"
    )
}
