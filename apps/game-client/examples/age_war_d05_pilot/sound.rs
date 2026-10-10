//! Optional cue playback for owner listening. Never affects the storyboard.
//!
//! Sound bytes cross `load_verified` like textures, then Macroquad decodes
//! them. Mix limits follow the D05 policy draft (family cooldown, a small
//! world-voice window); drops are counted, not hidden.

use std::collections::BTreeMap;

use macroquad::audio::{load_sound_from_bytes, play_sound, stop_sound, PlaySoundParams, Sound};

use crate::{load::Pack, scene::Cue};

#[derive(Clone, Debug)]
struct Row {
    id: String,
    group: String,
    phase: String,
    binding: String,
    pack: String,
}

fn rows(text: &str) -> Vec<Row> {
    text.lines()
        .filter(|line| line.starts_with("sfx\t"))
        .filter_map(|line| {
            let f: Vec<&str> = line.split('\t').collect();
            (f.len() == 11).then(|| Row {
                id: f[1].into(),
                group: f[2].into(),
                phase: f[3].into(),
                binding: f[4].into(),
                pack: f[10].into(),
            })
        })
        .collect()
}

/// Loaded sounds per cue family plus the current age loops.
#[derive(Debug, Default)]
pub struct Mixer {
    families: BTreeMap<String, Vec<Sound>>,
    loops: Vec<Sound>,
    last_played: BTreeMap<String, f32>,
    recent: Vec<f32>,
    turn: usize,
    volume: f32,
    pub played: u64,
    pub dropped: u64,
    pub loaded: usize,
}

impl Mixer {
    /// Loads the variations needed for `recipes` from the common pack and the
    /// age's ambience/motif loops from its own pack.
    pub async fn load(
        common: &Pack,
        age_pack: &Pack,
        age: &str,
        recipes: &[String],
        volume: f32,
    ) -> Result<Self, String> {
        let table = rows(&common.text("meta/sfx")?);
        let mut mixer = Self {
            volume,
            ..Self::default()
        };
        for recipe in recipes {
            let wanted: Vec<&Row> = table
                .iter()
                .filter(|row| {
                    row.pack == common.reference.pack().as_str()
                        && (row.id.starts_with(&format!("{recipe}_v"))
                            || (row.binding == *recipe && row.phase == "contact"))
                })
                .collect();
            for row in wanted {
                let bytes = common.verified(common.file_for(&format!("sfx/{}", row.id), 1)?)?;
                let sound = load_sound_from_bytes(bytes.bytes())
                    .await
                    .map_err(|e| format!("{}: {e:?}", row.id))?;
                mixer
                    .families
                    .entry(recipe.clone())
                    .or_default()
                    .push(sound);
                mixer.loaded += 1;
            }
        }
        for id in [format!("{age}_ambience_loop"), format!("{age}_motif_loop")] {
            if table
                .iter()
                .any(|row| row.id == id && row.group.starts_with("ambience"))
            {
                let bytes = age_pack.verified(age_pack.file_for(&format!("sfx/{id}"), 1)?)?;
                let sound = load_sound_from_bytes(bytes.bytes())
                    .await
                    .map_err(|e| format!("{id}: {e:?}"))?;
                play_sound(
                    &sound,
                    PlaySoundParams {
                        looped: true,
                        volume: volume * 0.25,
                    },
                );
                mixer.loops.push(sound);
                mixer.loaded += 1;
            }
        }
        Ok(mixer)
    }

    /// Plays one variation per cue, honouring a 70 ms family cooldown and at
    /// most 16 starts inside any 300 ms window.
    pub fn cue(&mut self, cue: &Cue, now_ms: f32) {
        let Some(sounds) = self.families.get(&cue.recipe) else {
            return;
        };
        self.recent.retain(|t| now_ms - t < 300.0);
        let cooled = self
            .last_played
            .get(&cue.recipe)
            .is_none_or(|t| now_ms - t >= 70.0);
        if !cooled || self.recent.len() >= 16 || sounds.is_empty() {
            self.dropped += 1;
            return;
        }
        self.turn = self.turn.wrapping_add(1);
        play_sound(
            &sounds[self.turn % sounds.len()],
            PlaySoundParams {
                looped: false,
                volume: self.volume * 0.6,
            },
        );
        self.last_played.insert(cue.recipe.clone(), now_ms);
        self.recent.push(now_ms);
        self.played += 1;
    }

    /// Interrupts the age loops (age change, pause or teardown).
    pub fn stop_loops(&mut self) {
        for sound in self.loops.drain(..) {
            stop_sound(&sound);
        }
    }
}
