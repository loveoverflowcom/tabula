//! Renderer-neutral one-shot audio contracts. (doc 04 §13)
//!
//! A cue is a pack-local semantic identity emitted from an authoritative
//! projected `ViewEvent`. Backends bind a sink to one already-loaded asset pack
//! and resolve that identity without making any loading or I/O decisions.

use smallvec::SmallVec;

/// A validated pack-local semantic identity for a one-shot presentation sound.
///
/// The owning [`crate::GamePresentation`] determines the cue meaning from a
/// projected event; [`AudioSink`] owns non-authoritative playback. The asset
/// pack returned by `GamePresentation::asset_pack` scopes this identity, so it
/// is deliberately not globally namespaced.
///
/// @ai.role presentation-value
/// @ai.domain presentation.audio
/// @ai.invariant nonempty-pack-local-cue-identity
/// @ai.evidence `tests::empty_cue_id_is_rejected`
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AudioCue {
    id: String,
}

impl AudioCue {
    /// Creates a cue only when its pack-local identity is non-empty.
    pub fn new(id: impl Into<String>) -> Result<Self, AudioCueError> {
        let id = id.into();
        if id.is_empty() {
            return Err(AudioCueError::EmptyId);
        }
        Ok(Self { id })
    }

    /// Creates a cue declared by this program.
    ///
    /// Static cue identities are developer-owned configuration. An invalid
    /// declaration is therefore a programming error, unlike an unavailable
    /// cue at an [`AudioSink`], which remains a non-authoritative runtime
    /// failure.
    #[track_caller]
    #[must_use]
    pub fn from_static(id: &'static str) -> Self {
        Self::new(id)
            .unwrap_or_else(|error| panic!("static audio cue declaration must be valid: {error:?}"))
    }

    /// Returns this cue's pack-local semantic identity.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }
}

/// Construction failure for [`AudioCue`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioCueError {
    /// A pack-local cue identity cannot be empty.
    EmptyId,
}

/// The small ordered collection of cues emitted for one projected view event.
///
/// A caller must preserve this order when handing cues to an [`AudioSink`].
pub type AudioCues = SmallVec<[AudioCue; 2]>;

/// A synchronous backend port for playing already-resolved, one-shot audio.
///
/// A sink is bound to the asset pack selected by the active game presentation.
/// It does not load, decode, fetch, or validate assets. Playback failure is
/// non-authoritative and must not affect a match, its projection, or input
/// processing (doc 00 I-10; doc 04 §13).
pub trait AudioSink {
    /// The backend-specific non-authoritative playback failure.
    type Error;

    /// Plays one already-resolved pack-local cue.
    fn play(&mut self, cue: &AudioCue) -> Result<(), Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::{AudioCue, AudioCueError};

    #[test]
    fn empty_cue_id_is_rejected() {
        assert_eq!(AudioCue::new(""), Err(AudioCueError::EmptyId));
        assert_eq!(AudioCue::new("move").unwrap().id(), "move");
    }

    #[test]
    #[should_panic(expected = "static audio cue declaration must be valid")]
    fn invalid_static_cue_declaration_fails_loudly() {
        let _ = AudioCue::from_static("");
    }
}
