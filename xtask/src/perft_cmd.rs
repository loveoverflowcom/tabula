//! `cargo xtask perft chess [depth]` — a convenient published-oracle probe.

use std::time::Instant;

use tabula_game_chess::{perft, State};

const INITIAL_FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
const EXPECTED: [u64; 6] = [1, 20, 400, 8_902, 197_281, 4_865_609];

pub(crate) fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(2);
    let game = args
        .next()
        .ok_or_else(|| "usage: cargo xtask perft chess [depth]".to_owned())?;
    if game != "chess" {
        return Err(format!(
            "unsupported game {game:?}; only chess has a perft oracle"
        ));
    }
    let depth = args
        .next()
        .map(|value| {
            value
                .parse::<u8>()
                .map_err(|_| "depth must be an integer".to_owned())
        })
        .transpose()?
        .unwrap_or(4);
    if args.next().is_some() {
        return Err("usage: cargo xtask perft chess [depth]".to_owned());
    }
    let expected = EXPECTED.get(usize::from(depth)).copied().ok_or_else(|| {
        "published initial-position counts are available through depth 5".to_owned()
    })?;
    let position = State::from_fen(INITIAL_FEN).map_err(|err| err.to_string())?;
    let started = Instant::now();
    let actual = perft(&position, depth);
    let status = if actual == expected { "PASS" } else { "FAIL" };
    println!("position: initial\ndepth: {depth}\nexpected nodes: {expected}\nactual nodes: {actual}\nelapsed: {:?}\n{status}", started.elapsed());
    if actual == expected {
        Ok(())
    } else {
        Err("perft count did not match the published oracle".to_owned())
    }
}
