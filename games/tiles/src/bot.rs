//! A local bot. (doc 02 §6)
//!
//! # It sees exactly what a player sees
//!
//! `choose` takes `&View`, not `&State` — so a bot that can play at all is
//! free evidence that the projection carries enough information to play
//! (doc 02 §6's first consequence). Everything this bot needs is public: the
//! board, the drawn tile, and the claim slots. If a future refactor moved
//! something the bot needs out of the `View`, this file would stop compiling,
//! which is a better alarm than a comment.
//!
//! # Why it is deliberately shallow
//!
//! Its job is to be a **fuzz driver**, not an opponent. `tabula-testkit`'s
//! self-play harness replays every match twice and compares the whole semantic
//! trace, so what matters is that the bot is deterministic given
//! `(view, rng)` and that it explores widely enough to reach late-game states,
//! completed features, and exhausted follower supplies. A strong evaluator
//! would explore *less*, because it would keep choosing the same lines.
//!
//! @ai.role bot-policy
//! @ai.domain tiles.bot
//! @ai.pure true
//! @ai.invariant bot-consumes-only-the-projection
//! @ai.evidence tests::self_play_terminates_deterministically_at_every_seat_count

#![allow(clippy::doc_markdown)]

use tabula_core::{BotLevel, DetRng, Millis, SeatId};
use tabula_game_api::GameBot;

use crate::rules::{legal_placements, Command, Status, TilesRules, TurnPhase, View};

/// How often the bot claims a feature when it could, out of sixteen.
///
/// Not 16/16: a bot that always claimed would spend its seven followers in the
/// first seven turns and then never exercise the claim path again. Not 0/16
/// either, or nothing would ever be scored. This value keeps followers
/// circulating for the whole match.
const CLAIM_CHANCE_IN_SIXTEEN: u32 = 10;

/// A shallow, deterministic policy.
#[derive(Debug)]
pub struct Greedy {
    level: BotLevel,
}

impl Greedy {
    #[must_use]
    pub const fn new(level: BotLevel) -> Self {
        Self { level }
    }
}

impl GameBot<TilesRules> for Greedy {
    fn level(&self) -> BotLevel {
        self.level
    }

    fn choose(&self, view: &View, seat: SeatId, rng: &mut DetRng) -> Option<Command> {
        if view.status != Status::Playing || view.paused || view.turn != seat {
            return None;
        }

        match view.phase {
            TurnPhase::PlaceTile => {
                let kind = view.drawn?;
                // Everything needed comes out of the projection: `legal_placements`
                // is the same function the rules use, over the same public board.
                let placements = legal_placements(&view.board, kind);
                if placements.is_empty() {
                    return None;
                }
                let square = pick(rng, placements.len())?;
                let (at, rotations) = placements.get(square)?;
                let rotation = *rotations.get(pick(rng, rotations.len())?)?;
                Some(Command::PlaceTile { at: *at, rotation })
            }
            TurnPhase::PlaceMeeple => {
                if view.meeple_slots.is_empty() || rng.below(16) >= CLAIM_CHANCE_IN_SIXTEEN {
                    return Some(Command::SkipMeeple);
                }
                let slot = pick(rng, view.meeple_slots.len())?;
                Some(Command::PlaceMeeple {
                    segment: *view.meeple_slots.get(slot)?,
                })
            }
        }
    }

    fn think_time(&self, _view: &View) -> Millis {
        Millis(300)
    }
}

/// A uniform index into a slice of `len` items, or `None` when it is empty.
fn pick(rng: &mut DetRng, len: usize) -> Option<usize> {
    if len == 0 {
        return None;
    }
    let bound = u32::try_from(len).unwrap_or(u32::MAX);
    Some(usize::try_from(rng.below(bound)).unwrap_or(0).min(len - 1))
}

#[cfg(test)]
mod tests {
    use tabula_core::{Occupant, SeatEntry, SeatRoster};
    use tabula_game_api::GameModule;
    use tabula_testkit::selfplay::{self, SelfPlayConfig, SelfPlaySetup};

    use super::*;
    use crate::rules::Config;
    use crate::TilesModule;

    fn roster(count: u8) -> SeatRoster {
        SeatRoster::new(
            (0..count)
                .map(|index| SeatEntry {
                    seat: SeatId(index),
                    occupant: Occupant::Bot {
                        level: BotLevel::Easy,
                    },
                    team: None,
                })
                .collect(),
        )
        .expect("fixture seats are unique")
    }

    #[test]
    fn the_module_offers_a_bot_for_the_levels_it_declares() {
        assert!(TilesModule::bot(BotLevel::Trivial).is_some());
        assert!(TilesModule::bot(BotLevel::Easy).is_some());
        // Declaring a level it cannot serve would be a lie the platform acts on.
        assert!(TilesModule::bot(BotLevel::Medium).is_none());
        assert!(TilesModule::bot(BotLevel::Hard).is_none());
    }

    /// **The primary fuzzer** (doc 02 §11.3). Each match is played twice from
    /// the same seed and the whole semantic trace is compared: input bytes,
    /// logical times, accept/reject results, per-input state hashes, events,
    /// effects, terminal outcome, final state bytes, and — with
    /// `check_projections` on — every viewer's `View` and `Option<ViewEvent>`
    /// at each checkpoint.
    ///
    /// It also injects hostile inputs at a fixed rate and checks each rejection
    /// is transactional, and fails a match that has not terminated. Between
    /// them that covers determinism, R2, and termination over a far wider slice
    /// of the reachable space than any hand-written script.
    ///
    /// The count is pinned low for the per-PR tier; `cargo xtask selfplay tiles
    /// --matches 100000` is the nightly campaign.
    #[test]
    fn self_play_terminates_deterministically_at_every_seat_count() {
        for seats in crate::rules::MIN_SEATS..=crate::rules::MAX_SEATS {
            let setup = SelfPlaySetup::<TilesRules> {
                config: Config {
                    turn_deadline_ms: 0,
                },
                roster: roster(seats),
            };
            let report = selfplay::run::<TilesModule>(
                &setup,
                &SelfPlayConfig {
                    matches: 12,
                    base_seed: [seats; 32],
                    hostile_fraction: 0.08,
                    max_inputs: 4_000,
                    check_projections: true,
                    start_match_index: 0,
                },
            )
            .expect("the setup and config are valid");

            assert!(
                report.is_success(),
                "{seats} seats: {} failure(s), first: {:?}",
                report.failures.len(),
                report.failures.first()
            );
            assert_eq!(report.terminated, report.matches_run);
            assert!(
                report.inputs_total > u64::from(report.matches_run) * 50,
                "{seats} seats: only {} inputs across {} matches — the matches ended \
                 far too early to have exercised a real board",
                report.inputs_total,
                report.matches_run
            );
        }
    }

    /// The bot must not be able to act when it is not its turn, and must not
    /// invent a command from a finished match — the platform routes its answer
    /// through the ordinary `Input::Player` path, where a stale command would
    /// be rejected, but returning one at all is a bug worth catching here.
    #[test]
    fn the_bot_declines_when_it_is_not_its_turn_or_the_match_is_over() {
        use tabula_core::{InputIndex, MatchSeed, Viewer};
        use tabula_game_api::{Budget, Ctx, GameRules};

        let seed = MatchSeed::from_bytes([1u8; 32]);
        let mut rng = DetRng::for_input(&seed, InputIndex(0));
        let mut ctx = Ctx {
            now: tabula_core::LogicalTime::ZERO,
            index: InputIndex(0),
            rng: &mut rng,
            budget: Budget::default(),
        };
        let state = TilesRules::create(
            &Config {
                turn_deadline_ms: 0,
            },
            &roster(3),
            &mut ctx,
        )
        .expect("valid setup")
        .state;

        let bot = Greedy::new(BotLevel::Easy);
        let view = TilesRules::project(&state, Viewer::Seat(SeatId(0)));
        let mut rng = DetRng::for_input(&seed, InputIndex(1));

        assert!(
            bot.choose(&view, SeatId(0), &mut rng).is_some(),
            "the seat on turn has something to do"
        );
        assert!(
            bot.choose(&view, SeatId(1), &mut rng).is_none(),
            "a seat that is not on turn must be offered nothing"
        );

        let mut paused = view.clone();
        paused.paused = true;
        assert!(bot.choose(&paused, SeatId(0), &mut rng).is_none());

        let mut over = view;
        over.status = Status::Ended;
        assert!(bot.choose(&over, SeatId(0), &mut rng).is_none());
    }
}
