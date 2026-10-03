//! Resolving a validated configuration into a real handoff.
//!
//! ADR-011: gameplay is a **separate document**, never a canvas mounted inside
//! the shell's DOM runtime. The shell therefore produces a URL and navigates;
//! it does not construct a match.
//!
//! A server match id is minted by the authority, which does not exist at this
//! phase (doc 07 Phase 4). This module never invents one: a local session
//! resolves to the literal `local` session document, and a network session is
//! not resolvable at all.

use crate::{availability::UnavailableReason, config::NormalizedConfig};

/// Where this build's gameplay document lives, if it is deployed at all.
///
/// `None` is the honest default: a shell build that was not given a gameplay
/// bundle cannot start a session, and says so rather than navigating nowhere.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RuntimeBinding {
    play_base: Option<&'static str>,
}

impl RuntimeBinding {
    /// No gameplay document is deployed with this shell.
    #[must_use]
    pub const fn unbound() -> Self {
        Self { play_base: None }
    }

    /// Bind the gameplay document base path, such as `/play`.
    #[must_use]
    pub const fn bound(play_base: &'static str) -> Self {
        Self {
            play_base: Some(play_base),
        }
    }

    #[must_use]
    pub const fn is_bound(&self) -> bool {
        self.play_base.is_some()
    }
}

/// A confirmed handoff target: a real document navigation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LaunchHandoff {
    pub url: String,
}

/// Resolve the handoff for a normalized configuration.
///
/// # Errors
/// [`UnavailableReason::NoGameplayRuntime`] when this build has no gameplay
/// document to hand off to. The caller keeps the reason visible instead of
/// reporting a started match.
pub fn resolve(
    binding: RuntimeBinding,
    config: &NormalizedConfig,
) -> Result<LaunchHandoff, UnavailableReason> {
    let base = binding
        .play_base
        .ok_or(UnavailableReason::NoGameplayRuntime)?;
    let query = config
        .launch_args
        .iter()
        .map(|(key, value)| format!("{}={}", encode(key), encode(value)))
        .collect::<Vec<_>>()
        .join("&");
    Ok(LaunchHandoff {
        url: format!("{}/local?{query}", base.trim_end_matches('/')),
    })
}

/// Percent-encode everything outside the unreserved set.
///
/// Launch arguments are adapter-produced configuration, never a credential or
/// a player identity (docs/ui/screens/discovery.md).
fn encode(value: &str) -> String {
    use core::fmt::Write as _;

    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(char::from(byte));
            }
            other => {
                let _ = write!(out, "%{other:02X}");
            }
        }
    }
    out
}
