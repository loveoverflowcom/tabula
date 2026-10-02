//! Tiles setup adapter. // xtask-allow-game-id: registry-owned game adapter.
//!
//! `Config::turn_deadline_ms` is `0` for no deadline, and any nonzero value is
//! required by the rules to be at least `MIN_TURN_DEADLINE_MS`. The form offers
//! seconds and lets the rules reject the values below that floor rather than
//! quietly lifting them to it.

use tabula_game_tiles::Config;

use crate::{
    availability::{LaunchMode, ModeSupport, UnavailableReason},
    config::{
        ChoiceSpec, ConfigDraft, ConfigForm, ConfigRejection, FieldKind, FieldSpec, SummaryLine,
        SummaryValue,
    },
    erased::GameSetup,
    i18n::{Locale, Messages},
    parse::{choice, integer},
};

const EN: Messages = &[
    ("game.tiles.name", "Tiles"),
    ("game.tiles.tagline", "Place tiles, claim features, score the map"),
    (
        "game.tiles.description",
        "A tile-laying game for two to five players: draw a tile, place it legally, and claim what it completes.",
    ),
    ("tiles.field.deadline", "Turn deadline"),
    (
        "tiles.field.deadline.hint",
        "Whether a seat must finish its turn within a time limit.",
    ),
    ("tiles.deadline.none", "No deadline"),
    ("tiles.deadline.timed", "Limited time per turn"),
    ("tiles.field.deadline_seconds", "Seconds per turn"),
    (
        "tiles.field.deadline_seconds.hint",
        "Whole seconds. The rules require at least five.",
    ),
];

const VI: Messages = &[
    ("game.tiles.name", "Xếp mảnh"),
    ("game.tiles.tagline", "Đặt mảnh, giành vùng, tính điểm bản đồ"),
    (
        "game.tiles.description",
        "Trò xếp mảnh cho hai đến năm người: rút một mảnh, đặt đúng luật và giành phần mình hoàn thành.",
    ),
    ("tiles.field.deadline", "Hạn mỗi lượt"),
    (
        "tiles.field.deadline.hint",
        "Mỗi ghế có phải hoàn thành lượt trong thời hạn hay không.",
    ),
    ("tiles.deadline.none", "Không giới hạn"),
    ("tiles.deadline.timed", "Giới hạn thời gian mỗi lượt"),
    ("tiles.field.deadline_seconds", "Số giây mỗi lượt"),
    (
        "tiles.field.deadline_seconds.hint",
        "Số giây nguyên. Luật chơi yêu cầu tối thiểu năm giây.",
    ),
];

const DEADLINE_OPTIONS: &[ChoiceSpec] = &[
    ChoiceSpec {
        value: "none",
        label_key: "tiles.deadline.none",
        reveals: &[],
    },
    ChoiceSpec {
        value: "timed",
        label_key: "tiles.deadline.timed",
        reveals: &["deadline_seconds"],
    },
];

const FIELDS: &[FieldSpec] = &[
    FieldSpec {
        key: "deadline",
        label_key: "tiles.field.deadline",
        hint_key: Some("tiles.field.deadline.hint"),
        kind: FieldKind::Choice {
            options: DEADLINE_OPTIONS,
        },
    },
    FieldSpec {
        key: "deadline_seconds",
        label_key: "tiles.field.deadline_seconds",
        hint_key: Some("tiles.field.deadline_seconds.hint"),
        kind: FieldKind::Integer {
            // One second is inside the form and outside the rules: the floor is
            // the game's to enforce, with its own message.
            min: 1,
            max: 86_400,
            default: 60,
        },
    },
];

const FORM: ConfigForm = ConfigForm { fields: FIELDS };

const MODES: &[ModeSupport] = &[
    ModeSupport::available(LaunchMode::LocalHotSeat),
    ModeSupport::available(LaunchMode::LocalBots),
    ModeSupport::unavailable(LaunchMode::Network, UnavailableReason::NoNetworkService),
];

#[derive(Debug)]
pub struct TilesSetup; // xtask-allow-game-id: registry-owned game adapter.

impl GameSetup for TilesSetup {
    type Module = tabula_game_tiles::TilesModule; // xtask-allow-game-id: registry-owned game adapter.

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

    /// The rules name the field `turn_deadline_ms`; the form offers seconds.
    fn field_key(module_field: &str) -> Option<&'static str> {
        (module_field == "turn_deadline_ms").then_some("deadline_seconds")
    }

    fn parse(
        draft: &ConfigDraft,
    ) -> Result<(Config, Vec<SummaryLine>, Vec<(String, String)>), ConfigRejection> {
        match choice(&FORM, "deadline", draft)? {
            "none" => Ok((
                Config {
                    turn_deadline_ms: 0,
                },
                vec![SummaryLine {
                    label_key: "setup.summary.deadline",
                    value: SummaryValue::Key("tiles.deadline.none"),
                }],
                vec![("deadline_ms".to_owned(), "0".to_owned())],
            )),
            "timed" => {
                let seconds = integer(&FORM, "deadline_seconds", draft)?;
                let millis = seconds.saturating_mul(1_000);
                Ok((
                    Config {
                        turn_deadline_ms: millis,
                    },
                    vec![SummaryLine {
                        label_key: "setup.summary.deadline",
                        value: SummaryValue::Millis(millis),
                    }],
                    vec![("deadline_ms".to_owned(), millis.to_string())],
                ))
            }
            _ => Err(ConfigRejection::field(
                "deadline",
                crate::config::RejectionReason::UnknownChoice,
            )),
        }
    }
}
