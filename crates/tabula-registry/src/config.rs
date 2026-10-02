//! Module-authored setup descriptors, drafts, and normalized results.
//!
//! The shell renders *descriptors*; it never names a configuration field of a
//! particular game and never parses a game's units (doc 04 §2.1, I-9). A game's
//! adapter owns field meaning, parsing, and the summary wording keys; the game's
//! own [`GameModule::validate_config`](tabula_game_api::GameModule::validate_config)
//! stays authoritative over the typed value the adapter produces.
//!
//! The split exists because `validate_config` takes a typed `Config` and returns
//! no normalized value (docs/ui/screens/discovery.md, "Typed data mapping"). The
//! adapter therefore builds the typed value from the draft first and calls
//! module validation once the seat plan is resolved.

use std::collections::BTreeMap;

/// A field a game's setup form offers, in the order the form presents it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FieldSpec {
    /// Stable key the shell uses for draft storage, DOM ids, and query state.
    pub key: &'static str,
    /// i18n key for the field's visible label.
    pub label_key: &'static str,
    /// i18n key for the unit/constraint hint announced with the field.
    pub hint_key: Option<&'static str>,
    pub kind: FieldKind,
}

/// What a field accepts. The shell maps each variant onto one native control.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldKind {
    /// One of a fixed set of module-named choices.
    Choice { options: &'static [ChoiceSpec] },
    /// A whole number in a module-defined inclusive range.
    ///
    /// The bounds are the *form's* bounds. They never replace module validation:
    /// a value inside them can still be rejected by the game. The unit is not a
    /// field here: it belongs to the game's own label and hint, and a second,
    /// platform-owned unit vocabulary would be a second source of truth.
    Integer {
        min: u64,
        max: u64,
        /// Value used when the draft has no entry yet.
        default: u64,
    },
}

/// One selectable value of a [`FieldKind::Choice`] field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChoiceSpec {
    pub value: &'static str,
    pub label_key: &'static str,
    /// Fields this choice makes meaningful; others stay hidden and unparsed.
    pub reveals: &'static [&'static str],
}

/// A game's complete setup form.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConfigForm {
    pub fields: &'static [FieldSpec],
}

impl ConfigForm {
    #[must_use]
    pub fn field(&self, key: &str) -> Option<&FieldSpec> {
        self.fields.iter().find(|field| field.key == key)
    }

    /// Fields that are meaningful for the current draft.
    ///
    /// A choice field hides the fields no selected option reveals, so an
    /// irrelevant value is never parsed, summarized, or reported as invalid.
    #[must_use]
    pub fn visible_fields(&self, draft: &ConfigDraft) -> Vec<&FieldSpec> {
        let mut revealed: Vec<&'static str> = Vec::new();
        let mut hidden: Vec<&'static str> = Vec::new();
        for field in self.fields {
            if let FieldKind::Choice { options } = field.kind {
                let selected = draft.get(field.key).or(options.first().map(|o| o.value));
                for option in options {
                    for key in option.reveals {
                        if Some(option.value) == selected {
                            revealed.push(key);
                        } else {
                            hidden.push(key);
                        }
                    }
                }
            }
        }
        self.fields
            .iter()
            .filter(|field| revealed.contains(&field.key) || !hidden.contains(&field.key))
            .collect()
    }
}

/// Raw, unparsed field values exactly as the player entered them.
///
/// The draft is deliberately textual: keeping the entered text is what lets a
/// rejection preserve the input it rejected (docs/ui/screens/03-new-match.md).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ConfigDraft {
    values: BTreeMap<String, String>,
}

impl ConfigDraft {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A draft pre-filled with the form's defaults, used for a fresh entry.
    #[must_use]
    pub fn with_defaults(form: &ConfigForm) -> Self {
        let mut draft = Self::new();
        for field in form.fields {
            match field.kind {
                FieldKind::Choice { options } => {
                    if let Some(first) = options.first() {
                        draft.set(field.key, first.value);
                    }
                }
                FieldKind::Integer { default, .. } => {
                    draft.set(field.key, default.to_string());
                }
            }
        }
        draft
    }

    pub fn set(&mut self, key: &str, value: impl Into<String>) {
        self.values.insert(key.to_owned(), value.into());
    }

    #[must_use]
    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }
}

/// Why a draft is not a usable configuration.
///
/// `field` is the form key the shell focuses and marks `aria-invalid`; the
/// reason carries the i18n key of the message, never developer text
/// (docs/ui/screens/discovery.md, "Configuration and creation states").
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigRejection {
    pub field: Option<String>,
    pub reason: RejectionReason,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RejectionReason {
    /// The field has no value and the game requires one.
    Missing,
    /// The entered text is not a whole number in this unit.
    NotANumber,
    /// A whole number outside the form's declared range.
    OutOfRange { min: u64, max: u64 },
    /// A choice value the form does not offer.
    UnknownChoice,
    /// The game rejected the seat count for this plan.
    SeatCount,
    /// The game rejected the typed value this field produced.
    ModuleField,
    /// The game rejected the combination rather than one field.
    Unsupported,
}

impl RejectionReason {
    /// Stable i18n key; the shell owns en/vi wording (doc 07 §i18n).
    #[must_use]
    pub const fn message_key(&self) -> &'static str {
        match self {
            Self::Missing => "setup.reject.missing",
            Self::NotANumber => "setup.reject.not_a_number",
            Self::OutOfRange { .. } => "setup.reject.out_of_range",
            Self::UnknownChoice => "setup.reject.unknown_choice",
            Self::SeatCount => "setup.reject.seat_count",
            Self::ModuleField => "setup.reject.module_field",
            Self::Unsupported => "setup.reject.unsupported",
        }
    }
}

impl ConfigRejection {
    #[must_use]
    pub fn field(key: &str, reason: RejectionReason) -> Self {
        Self {
            field: Some(key.to_owned()),
            reason,
        }
    }

    #[must_use]
    pub const fn whole(reason: RejectionReason) -> Self {
        Self {
            field: None,
            reason,
        }
    }
}

/// One line of the normalized summary shown immediately before the CTA.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SummaryLine {
    pub label_key: &'static str,
    pub value: SummaryValue,
}

/// A normalized value, kept structured so the shell can localize it without
/// concatenating fragments and without flattening distinct semantics.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SummaryValue {
    /// A module-chosen i18n key, such as an untimed clock.
    Key(&'static str),
    Count(u64),
    Seats {
        count: u64,
    },
    /// A clock the module defines, with its control kept distinct: an
    /// increment and a delay never render as the same label.
    TimeControl {
        initial_ms: u64,
        control: TimeControlKind,
    },
    Millis(u64),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimeControlKind {
    Untimed,
    Increment { millis: u64 },
    Delay { millis: u64 },
}

/// A draft that parsed, passed its game's validation for the resolved seat
/// plan, and can be submitted exactly as summarized.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NormalizedConfig {
    pub summary: Vec<SummaryLine>,
    /// Canonical, adapter-produced launch arguments for the gameplay document.
    ///
    /// The shell forwards these verbatim; it does not read or rewrite them, so
    /// no game-specific key reaches shell code (I-9).
    pub launch_args: Vec<(String, String)>,
}
