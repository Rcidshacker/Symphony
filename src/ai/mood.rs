//! Mood matching from measured audio features.
//!
//! Tags are rarely present, so a mood such as "energetic" is matched against how loud and
//! how fast a track is *relative to the rest of the library*: ranking by percentile makes
//! the result independent of how loud a given master happens to be.

use crate::audio::features::AudioFeatures;

/// Beat strengths below this are treated as "no reliable tempo"
const MIN_BEAT_STRENGTH: f32 = 0.04;

/// Where on a scale (library percentile) a mood wants a feature to sit
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Target {
    Low,
    Mid,
    High,
}

impl Target {
    /// 1.0 when `percentile` (0..=1) is where the target wants it, falling to 0.0 away from it
    fn fit(self, percentile: f32) -> f32 {
        match self {
            Target::Low => 1.0 - percentile,
            Target::High => percentile,
            Target::Mid => 1.0 - (percentile - 0.5).abs() * 2.0,
        }
    }
}

/// What a mood asks of the audio
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MoodProfile {
    pub energy: Target,
    pub tempo: Target,
}

/// Profile for a mood word; `None` for words that say nothing about the sound
pub fn mood_profile(mood: &str) -> Option<MoodProfile> {
    use Target::{High, Low, Mid};
    let (energy, tempo) = match mood.trim().to_lowercase().as_str() {
        "energetic" | "pumped" | "workout" | "gym" | "hype" | "upbeat" | "party" | "dance" => (High, High),
        "aggressive" | "angry" | "intense" => (High, High),
        "happy" | "joyful" | "cheerful" | "fun" => (High, Mid),
        "epic" | "dramatic" | "powerful" => (High, Mid),
        "calm" | "focus" | "concentrate" | "study" | "coding" | "work" | "relaxed" | "relaxing" | "peaceful" | "gentle" | "sleep" | "chill" | "laid-back" => (Low, Low),
        "sad" | "melancholic" | "melancholy" | "blue" => (Low, Low),
        "romantic" | "love" => (Low, Low),
        _ => return None,
    };
    Some(MoodProfile { energy, tempo })
}

/// Sorted energies and tempos of a set of tracks, to turn raw values into percentiles
pub struct Ranks {
    energies: Vec<f32>,
    bpms: Vec<f32>,
}

impl Ranks {
    /// Ranks over the given features (tracks without a usable tempo do not count towards tempo)
    pub fn new<'a>(features: impl Iterator<Item = &'a AudioFeatures>) -> Self {
        let mut energies = Vec::new();
        let mut bpms = Vec::new();
        for f in features {
            energies.push(f.energy);
            if has_tempo(f) {
                bpms.push(f.bpm);
            }
        }
        energies.sort_by(f32::total_cmp);
        bpms.sort_by(f32::total_cmp);
        Self { energies, bpms }
    }

    /// Share of tracks quieter than `value` (0.5 when there is nothing to compare with)
    fn percentile(sorted: &[f32], value: f32) -> f32 {
        if sorted.len() < 2 {
            return 0.5;
        }
        let below = sorted.partition_point(|v| *v < value);
        below as f32 / (sorted.len() - 1) as f32
    }

    /// How well a track's sound fits `profile`: 0.0 to 1.0, 0.5 when nothing is known
    pub fn mood_fit(&self, profile: MoodProfile, features: &AudioFeatures) -> f32 {
        let energy = profile
            .energy
            .fit(Self::percentile(&self.energies, features.energy).min(1.0));
        if has_tempo(features) {
            let tempo = profile
                .tempo
                .fit(Self::percentile(&self.bpms, features.bpm).min(1.0));
            (energy + tempo) / 2.0
        } else {
            energy
        }
    }
}

/// Whether the tempo estimate is believable
pub fn has_tempo(features: &AudioFeatures) -> bool {
    features.bpm > 0.0 && features.beat_strength >= MIN_BEAT_STRENGTH
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(energy: f32, bpm: f32) -> AudioFeatures {
        AudioFeatures { energy, bpm, beat_strength: 0.3 }
    }

    #[test]
    fn known_moods_have_profiles_and_unknown_words_do_not() {
        assert!(mood_profile("Energetic").is_some());
        assert!(mood_profile(" chill ").is_some());
        assert_eq!(mood_profile("purple"), None);
    }

    #[test]
    fn energetic_prefers_loud_fast_tracks_and_calm_the_opposite() {
        let library = [f(0.2, 70.0), f(0.5, 100.0), f(0.9, 150.0)];
        let ranks = Ranks::new(library.iter());
        let energetic = mood_profile("energetic").unwrap();
        let calm = mood_profile("calm").unwrap();

        let scores = |p| library.iter().map(|t| ranks.mood_fit(p, t)).collect::<Vec<_>>();
        let (e, c) = (scores(energetic), scores(calm));
        assert!(e[2] > e[1] && e[1] > e[0], "energetic: {e:?}");
        assert!(c[0] > c[1] && c[1] > c[2], "calm: {c:?}");
        assert_eq!((e[2], c[0]), (1.0, 1.0));
    }

    #[test]
    fn mid_targets_prefer_the_middle_of_the_library() {
        let library = [f(0.1, 60.0), f(0.5, 100.0), f(0.9, 160.0)];
        let ranks = Ranks::new(library.iter());
        let happy = mood_profile("happy").unwrap(); // high energy, mid tempo
        let scores: Vec<f32> = library.iter().map(|t| ranks.mood_fit(happy, t)).collect();
        assert!(scores[2] > scores[0], "{scores:?}");
    }

    #[test]
    fn tracks_without_a_reliable_tempo_are_judged_on_energy_alone() {
        let ambient = AudioFeatures { energy: 0.1, bpm: 0.0, beat_strength: 0.0 };
        let library = [ambient, f(0.9, 150.0), f(0.6, 120.0)];
        let ranks = Ranks::new(library.iter());
        let calm = mood_profile("calm").unwrap();
        assert_eq!(ranks.mood_fit(calm, &ambient), 1.0);
        assert!(!has_tempo(&ambient));
    }

    #[test]
    fn a_single_track_is_neutral() {
        let only = f(0.7, 100.0);
        let ranks = Ranks::new([only].iter());
        assert_eq!(ranks.mood_fit(mood_profile("energetic").unwrap(), &only), 0.5);
    }
}
