//! Draft parsing shared by game adapters.
//!
//! Parsing is deliberately strict: a value is either exactly what the field
//! declares or a rejection that names the field. Nothing is clamped, rounded,
//! or coerced, because a silently corrected setup value is a configuration the
//! player never chose (docs/ui/screens/03-new-match.md).

use crate::config::{ConfigDraft, ConfigForm, ConfigRejection, FieldKind, RejectionReason};

/// Resolve a choice field to one of its declared option values.
///
/// # Errors
/// [`RejectionReason::Missing`] when the draft has no value, or
/// [`RejectionReason::UnknownChoice`] when it is not an offered option.
pub(crate) fn choice(
    form: &'static ConfigForm,
    key: &'static str,
    draft: &ConfigDraft,
) -> Result<&'static str, ConfigRejection> {
    let field = form
        .field(key)
        .expect("an adapter only reads fields its own form declares");
    let FieldKind::Choice { options } = field.kind else {
        unreachable!("{key} is declared as a choice field")
    };
    let raw = draft
        .get(key)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ConfigRejection::field(key, RejectionReason::Missing))?;
    options
        .iter()
        .find(|option| option.value == raw)
        .map(|option| option.value)
        .ok_or_else(|| ConfigRejection::field(key, RejectionReason::UnknownChoice))
}

/// Resolve an integer field to a whole number inside its declared range.
///
/// # Errors
/// [`RejectionReason::Missing`], [`RejectionReason::NotANumber`] for anything
/// that is not a bare non-negative whole number (a sign, a fraction, a unit
/// suffix, or an overflowing literal), or [`RejectionReason::OutOfRange`].
pub(crate) fn integer(
    form: &'static ConfigForm,
    key: &'static str,
    draft: &ConfigDraft,
) -> Result<u64, ConfigRejection> {
    let field = form
        .field(key)
        .expect("an adapter only reads fields its own form declares");
    let FieldKind::Integer { min, max, .. } = field.kind else {
        unreachable!("{key} is declared as an integer field")
    };
    let raw = draft
        .get(key)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ConfigRejection::field(key, RejectionReason::Missing))?;
    if !raw.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(ConfigRejection::field(key, RejectionReason::NotANumber));
    }
    let value: u64 = raw
        .parse()
        .map_err(|_| ConfigRejection::field(key, RejectionReason::NotANumber))?;
    if value < min || value > max {
        return Err(ConfigRejection::field(
            key,
            RejectionReason::OutOfRange { min, max },
        ));
    }
    Ok(value)
}
