//! Macroquad playback adapter for renderer-neutral audio cues. (doc 04 §13)

use std::collections::BTreeMap;

use macroquad::{audio, audio::Sound};
use tabula_presentation::{AudioCue, AudioSink};

/// Game-agnostic Macroquad audio sink bound to one preloaded asset pack.
///
/// Sound loading belongs to `tabula-assets` in Phase 3. This type only accepts
/// handles that a future asset layer has already loaded and maps pack-local cue
/// IDs to them. It intentionally knows no game IDs or asset paths.
#[derive(Debug, Default)]
pub struct MacroquadAudioSink {
    sounds: CueRegistry<Sound>,
}

impl MacroquadAudioSink {
    /// Creates an empty sink for the active presentation's asset pack.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers one already-loaded sound handle.
    ///
    /// Duplicate cue registration is rejected; replacing a loaded handle is a
    /// future asset-lifecycle concern rather than an implicit playback policy.
    pub fn register(&mut self, cue: AudioCue, sound: Sound) -> Result<(), MacroquadAudioError> {
        self.sounds
            .register(cue, sound)
            .map_err(MacroquadAudioError::DuplicateCue)
    }
}

impl AudioSink for MacroquadAudioSink {
    type Error = MacroquadAudioError;

    fn play(&mut self, cue: &AudioCue) -> Result<(), Self::Error> {
        let sound = self
            .sounds
            .resolve(cue)
            .ok_or_else(|| MacroquadAudioError::CueUnavailable(cue.clone()))?;
        audio::play_sound_once(sound);
        Ok(())
    }
}

/// Non-authoritative failure from [`MacroquadAudioSink`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MacroquadAudioError {
    /// No preloaded handle was registered for the cue in the active pack.
    CueUnavailable(AudioCue),
    /// A cue can be registered only once for one sink/pack binding.
    DuplicateCue(AudioCue),
}

/// Deterministic, game-agnostic pack-local cue registry.
#[derive(Debug)]
struct CueRegistry<T> {
    entries: BTreeMap<AudioCue, T>,
}

impl<T> Default for CueRegistry<T> {
    fn default() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }
}

impl<T> CueRegistry<T> {
    fn register(&mut self, cue: AudioCue, value: T) -> Result<(), AudioCue> {
        if self.entries.contains_key(&cue) {
            return Err(cue);
        }
        self.entries.insert(cue, value);
        Ok(())
    }

    fn resolve(&self, cue: &AudioCue) -> Option<&T> {
        self.entries.get(cue)
    }
}

#[cfg(test)]
mod tests {
    use super::{CueRegistry, MacroquadAudioError, MacroquadAudioSink};
    use tabula_presentation::AudioCue;
    use tabula_presentation::AudioSink;

    fn cue(id: &str) -> AudioCue {
        AudioCue::new(id).expect("test cue IDs are non-empty")
    }

    #[test]
    fn registered_cue_resolves_deterministically() {
        let mut registry = CueRegistry::default();
        registry.register(cue("capture"), 2_u8).unwrap();
        registry.register(cue("move"), 1_u8).unwrap();

        assert_eq!(registry.resolve(&cue("move")), Some(&1));
        assert_eq!(registry.resolve(&cue("capture")), Some(&2));
    }

    #[test]
    fn unknown_cue_is_unavailable_without_a_panic() {
        let registry = CueRegistry::<u8>::default();
        assert_eq!(registry.resolve(&cue("move")), None);
    }

    #[test]
    fn sink_reports_unknown_cues_as_a_structured_failure() {
        let mut sink = MacroquadAudioSink::new();
        assert_eq!(
            sink.play(&cue("move")),
            Err(MacroquadAudioError::CueUnavailable(cue("move")))
        );
    }

    #[test]
    fn duplicate_registration_is_rejected_without_replacing_the_handle() {
        let mut registry = CueRegistry::default();
        registry.register(cue("move"), 1_u8).unwrap();

        assert_eq!(registry.register(cue("move"), 2_u8), Err(cue("move")));
        assert_eq!(registry.resolve(&cue("move")), Some(&1));
    }
}
