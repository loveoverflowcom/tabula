//! Chess setup adapter. // xtask-allow-game-id: registry-owned game adapter.
//!
//! Field meaning comes from the rules: `Config::clock` is `None` for untimed
//! play, and a timed config carries an initial budget with either a Fischer
//! increment or a Bronstein delay. The two controls are summarized separately
//! because they are different promises to the player.

use tabula_game_chess::{ClockConfig, ClockControl, Config};

use tabula_core::Millis;

use crate::{
    availability::{LaunchMode, ModeSupport, UnavailableReason},
    config::{
        ChoiceSpec, ConfigDraft, ConfigForm, ConfigRejection, FieldKind, FieldSpec, SummaryLine,
        SummaryValue, TimeControlKind,
    },
    erased::GameSetup,
    i18n::{Locale, Messages},
    parse::{choice, integer},
};

const EN: Messages = &[
    ("game.chess.name", "Chess"),
    ("game.chess.tagline", "The classic two-player strategy game"),
    (
        "game.chess.description",
        "Standard chess: every legal move, check, stalemate, the fifty-move rule and threefold repetition, with optional clocks.",
    ),
    ("chess.field.clock", "Clock"),
    ("chess.field.clock.hint", "How each player's time is kept."),
    ("chess.clock.untimed", "No clock"),
    ("chess.clock.fischer", "Increment (Fischer)"),
    ("chess.clock.bronstein", "Delay (Bronstein)"),
    ("chess.field.initial_minutes", "Starting time"),
    (
        "chess.field.initial_minutes.hint",
        "Whole minutes on each player's clock. The rules reject a start of zero.",
    ),
    ("chess.field.increment_seconds", "Increment"),
    (
        "chess.field.increment_seconds.hint",
        "Whole seconds added to a player's clock after each of their moves.",
    ),
    ("chess.field.delay_seconds", "Delay"),
    (
        "chess.field.delay_seconds.hint",
        "Whole seconds of each move that do not come off the clock.",
    ),
];

const VI: Messages = &[
    ("game.chess.name", "Cờ vua"),
    ("game.chess.tagline", "Trò chơi chiến thuật hai người kinh điển"),
    (
        "game.chess.description",
        "Cờ vua tiêu chuẩn: đủ nước đi hợp lệ, chiếu, hết nước đi, luật năm mươi nước và lặp ba lần, có thể bật đồng hồ.",
    ),
    ("chess.field.clock", "Đồng hồ"),
    ("chess.field.clock.hint", "Cách tính giờ cho mỗi người chơi."),
    ("chess.clock.untimed", "Không tính giờ"),
    ("chess.clock.fischer", "Cộng giờ (Fischer)"),
    ("chess.clock.bronstein", "Hoãn giờ (Bronstein)"),
    ("chess.field.initial_minutes", "Thời gian ban đầu"),
    (
        "chess.field.initial_minutes.hint",
        "Số phút nguyên cho đồng hồ mỗi người. Luật chơi từ chối mức khởi đầu bằng không.",
    ),
    ("chess.field.increment_seconds", "Giờ cộng"),
    (
        "chess.field.increment_seconds.hint",
        "Số giây nguyên được cộng vào đồng hồ sau mỗi nước của người đó.",
    ),
    ("chess.field.delay_seconds", "Giờ hoãn"),
    (
        "chess.field.delay_seconds.hint",
        "Số giây nguyên đầu mỗi nước không bị trừ vào đồng hồ.",
    ),
];

const CLOCK_OPTIONS: &[ChoiceSpec] = &[
    ChoiceSpec {
        value: "untimed",
        label_key: "chess.clock.untimed",
        reveals: &[],
    },
    ChoiceSpec {
        value: "fischer",
        label_key: "chess.clock.fischer",
        reveals: &["initial_minutes", "increment_seconds"],
    },
    ChoiceSpec {
        value: "bronstein",
        label_key: "chess.clock.bronstein",
        reveals: &["initial_minutes", "delay_seconds"],
    },
];

const FIELDS: &[FieldSpec] = &[
    FieldSpec {
        key: "clock",
        label_key: "chess.field.clock",
        hint_key: Some("chess.field.clock.hint"),
        kind: FieldKind::Choice {
            options: CLOCK_OPTIONS,
        },
    },
    FieldSpec {
        key: "initial_minutes",
        label_key: "chess.field.initial_minutes",
        hint_key: Some("chess.field.initial_minutes.hint"),
        kind: FieldKind::Integer {
            // Zero is accepted by the form and rejected by the rules: the form
            // must not pre-empt the game's own validation of its clock.
            min: 0,
            max: 600,
            default: 5,
        },
    },
    FieldSpec {
        key: "increment_seconds",
        label_key: "chess.field.increment_seconds",
        hint_key: Some("chess.field.increment_seconds.hint"),
        kind: FieldKind::Integer {
            min: 0,
            max: 180,
            default: 2,
        },
    },
    FieldSpec {
        key: "delay_seconds",
        label_key: "chess.field.delay_seconds",
        hint_key: Some("chess.field.delay_seconds.hint"),
        kind: FieldKind::Integer {
            min: 0,
            max: 180,
            default: 2,
        },
    },
];

const FORM: ConfigForm = ConfigForm { fields: FIELDS };

/// Chess builds a bot for Trivial and Easy, and this crate links that feature,
/// so the bot mode has a real construction path. No online authority exists at
/// this phase, so the network mode names that as its reason.
const MODES: &[ModeSupport] = &[
    ModeSupport::available(LaunchMode::LocalHotSeat),
    ModeSupport::available(LaunchMode::LocalBots),
    ModeSupport::unavailable(LaunchMode::Network, UnavailableReason::NoNetworkService),
];

#[derive(Debug)]
pub struct ChessSetup; // xtask-allow-game-id: registry-owned game adapter.

impl GameSetup for ChessSetup {
    type Module = tabula_game_chess::ChessModule; // xtask-allow-game-id: registry-owned game adapter.

    fn form() -> &'static ConfigForm {
        &FORM
    }

    fn modes() -> &'static [ModeSupport] {
        MODES
    }

    fn messages(locale: Locale) -> Messages {
        match locale {
            Locale::En => EN,
            Locale::Vi => VI,
        }
    }

    fn parse(
        draft: &ConfigDraft,
    ) -> Result<(Config, Vec<SummaryLine>, Vec<(String, String)>), ConfigRejection> {
        let selected = choice(&FORM, "clock", draft)?;
        match selected {
            "untimed" => Ok((
                Config { clock: None },
                vec![SummaryLine {
                    label_key: "setup.summary.time",
                    value: SummaryValue::TimeControl {
                        initial_ms: 0,
                        control: TimeControlKind::Untimed,
                    },
                }],
                vec![("clock".to_owned(), "untimed".to_owned())],
            )),
            "fischer" => {
                let initial_ms = minutes_to_millis(integer(&FORM, "initial_minutes", draft)?);
                let increment_ms = seconds_to_millis(integer(&FORM, "increment_seconds", draft)?);
                Ok((
                    Config {
                        clock: Some(ClockConfig {
                            initial: Millis(initial_ms),
                            control: ClockControl::Fischer {
                                increment: Millis(increment_ms),
                            },
                        }),
                    },
                    vec![SummaryLine {
                        label_key: "setup.summary.time",
                        value: SummaryValue::TimeControl {
                            initial_ms,
                            control: TimeControlKind::Increment {
                                millis: increment_ms,
                            },
                        },
                    }],
                    vec![
                        ("clock".to_owned(), "fischer".to_owned()),
                        ("initial_ms".to_owned(), initial_ms.to_string()),
                        ("increment_ms".to_owned(), increment_ms.to_string()),
                    ],
                ))
            }
            "bronstein" => {
                let initial_ms = minutes_to_millis(integer(&FORM, "initial_minutes", draft)?);
                let delay_ms = seconds_to_millis(integer(&FORM, "delay_seconds", draft)?);
                Ok((
                    Config {
                        clock: Some(ClockConfig {
                            initial: Millis(initial_ms),
                            control: ClockControl::Bronstein {
                                delay: Millis(delay_ms),
                            },
                        }),
                    },
                    vec![SummaryLine {
                        label_key: "setup.summary.time",
                        value: SummaryValue::TimeControl {
                            initial_ms,
                            control: TimeControlKind::Delay { millis: delay_ms },
                        },
                    }],
                    vec![
                        ("clock".to_owned(), "bronstein".to_owned()),
                        ("initial_ms".to_owned(), initial_ms.to_string()),
                        ("delay_ms".to_owned(), delay_ms.to_string()),
                    ],
                ))
            }
            _ => Err(ConfigRejection::field(
                "clock",
                crate::config::RejectionReason::UnknownChoice,
            )),
        }
    }
}

/// The form's maximum is 600 minutes plus 180 seconds, so neither conversion
/// can overflow `u64` milliseconds; `saturating_mul` keeps that total anyway.
fn minutes_to_millis(minutes: u64) -> u64 {
    minutes.saturating_mul(60_000)
}

fn seconds_to_millis(seconds: u64) -> u64 {
    seconds.saturating_mul(1_000)
}
