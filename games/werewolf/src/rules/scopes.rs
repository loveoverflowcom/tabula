//! Absolute communication permissions as pure data (W-D14, doc 00 §6.3).
//! Voice transport remains gated: symmetric VoiceRoom cannot express dead listen-only.
use super::{Phase, State};
use smallvec::SmallVec;
use tabula_core::SeatId;
use tabula_game_api::effect::{ChannelScope, Participants};
use tabula_game_api::ChatScopes;
fn seats(values: impl Iterator<Item = SeatId>) -> Participants {
    Participants::Seats(values.collect::<SmallVec<[SeatId; 8]>>())
}
/// Phase-specific chat scope facts, with no transport enforcement claim.
#[must_use]
pub fn chat_scopes(state: &State) -> ChatScopes {
    let living = || seats(state.alive.iter().copied());
    let dead = || {
        seats(
            state
                .roster
                .iter()
                .copied()
                .filter(|seat| !state.alive.contains(seat)),
        )
    };
    let wolves = || {
        seats(
            state
                .alive
                .iter()
                .copied()
                .filter(|seat| state.roles[seat].is_wolf()),
        )
    };
    let (speak, listen) = match state.phase {
        Phase::Night | Phase::Vote => (Participants::None, Participants::None),
        Phase::Dawn | Phase::Dusk => (Participants::None, seats(state.roster.iter().copied())),
        Phase::Day => (living(), living()),
        Phase::Ended => (
            seats(state.roster.iter().copied()),
            seats(state.roster.iter().copied()),
        ),
    };
    ChatScopes {
        channels: smallvec::smallvec![
            ChannelScope {
                key: "table".into(),
                speak,
                listen
            },
            ChannelScope {
                key: "wolves".into(),
                speak: if state.phase == Phase::Night {
                    wolves()
                } else {
                    Participants::None
                },
                listen: if state.phase == Phase::Night {
                    wolves()
                } else {
                    Participants::None
                }
            },
            ChannelScope {
                key: "dead".into(),
                speak: dead(),
                listen: dead()
            },
        ],
    }
}
