//! "More like this": suggestions that combine tag similarity (artist, genre, year) with how the
//! tracks actually sound (loudness and tempo), so untagged libraries still get sensible results.

use std::collections::HashMap;

use super::mood::has_tempo;
use super::recommendations::RecommendationEngine;
use crate::app::Track;
use crate::audio::features::AudioFeatures;

/// A suggested track with its score (0..=1) and a short reason
#[derive(Debug, Clone)]
pub struct Suggestion {
    pub track: Track,
    pub score: f32,
    pub reason: String,
}

fn features_of<'a>(
    track: &Track,
    features: &'a HashMap<String, AudioFeatures>,
) -> Option<&'a AudioFeatures> {
    features.get(track.path.to_string_lossy().as_ref())
}

/// How alike two tracks sound, 0.0 to 1.0 (`None` when either has not been analyzed)
pub fn sound_similarity(a: &AudioFeatures, b: &AudioFeatures) -> f32 {
    let energy_gap = (a.energy - b.energy).abs();
    if has_tempo(a) && has_tempo(b) {
        let tempo_gap = ((a.bpm - b.bpm).abs() / 60.0).min(1.0);
        1.0 - (energy_gap + tempo_gap) / 2.0
    } else {
        1.0 - energy_gap
    }
}

/// Up to `limit` tracks from `library` most like `seed`, best first
pub fn suggest(
    seed: &Track,
    library: &[Track],
    features: &HashMap<String, AudioFeatures>,
    limit: usize,
) -> Vec<Suggestion> {
    // Tag similarity: only tracks the engine finds similar get a score
    let by_tags: HashMap<String, (f32, String)> = RecommendationEngine::new()
        .get_similar(seed, library, library.len())
        .into_iter()
        .map(|r| (r.track.id, (r.score, r.reason)))
        .collect();
    let seed_features = features_of(seed, features);

    let mut suggestions: Vec<Suggestion> = library
        .iter()
        .filter(|t| t.id != seed.id)
        .filter_map(|track| {
            let tags = by_tags.get(&track.id);
            let sound = seed_features
                .zip(features_of(track, features))
                .map(|(a, b)| sound_similarity(a, b));

            let (score, reason) = match (tags, sound) {
                (Some((tag_score, tag_reason)), Some(sound)) => (
                    0.5 * tag_score + 0.5 * sound,
                    format!("{tag_reason}, similar sound"),
                ),
                (Some((tag_score, tag_reason)), None) => (*tag_score, tag_reason.clone()),
                (None, Some(sound)) => (0.8 * sound, "similar sound".to_string()),
                (None, None) => return None,
            };
            Some(Suggestion {
                track: track.clone(),
                score,
                reason,
            })
        })
        .collect();

    suggestions.sort_by(|a, b| b.score.total_cmp(&a.score));
    suggestions.truncate(limit);
    suggestions
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::TrackSource;
    use std::path::PathBuf;

    fn track(id: &str, artist: &str, genre: Option<&str>) -> Track {
        Track {
            id: id.into(),
            title: id.into(),
            artist: artist.into(),
            album: "Album".into(),
            duration: 200.0,
            path: PathBuf::from(format!("/m/{id}.mp3")),
            track_number: None,
            genre: genre.map(String::from),
            year: None,
            source: TrackSource::Local,
        }
    }

    fn feature_map(entries: &[(&str, f32, f32)]) -> HashMap<String, AudioFeatures> {
        entries
            .iter()
            .map(|(id, energy, bpm)| {
                (
                    format!("/m/{id}.mp3"),
                    AudioFeatures {
                        energy: *energy,
                        bpm: *bpm,
                        beat_strength: 0.4,
                    },
                )
            })
            .collect()
    }

    #[test]
    fn untagged_tracks_are_matched_by_sound() {
        let library = vec![
            track("seed", "A", None),
            track("fast-loud", "B", None),
            track("slow-quiet", "C", None),
            track("unanalyzed", "D", None),
        ];
        let features = feature_map(&[
            ("seed", 0.9, 150.0),
            ("fast-loud", 0.85, 145.0),
            ("slow-quiet", 0.2, 70.0),
        ]);

        let got = suggest(&library[0], &library, &features, 10);
        let ids: Vec<_> = got.iter().map(|s| s.track.id.as_str()).collect();
        assert_eq!(
            ids,
            vec!["fast-loud", "slow-quiet"],
            "no seed in the list, unanalyzed tracks have nothing to go on"
        );
        assert!(got[0].score > 0.7 && got[1].score < 0.4, "{got:?}");
        assert_eq!(got[0].reason, "similar sound");
    }

    #[test]
    fn tags_and_sound_combine() {
        let library = vec![
            track("seed", "Same Artist", Some("rock")),
            track("same-artist-quiet", "Same Artist", Some("rock")),
            track("other-loud", "Other", Some("jazz")),
        ];
        let features = feature_map(&[
            ("seed", 0.9, 150.0),
            ("same-artist-quiet", 0.3, 80.0),
            ("other-loud", 0.9, 150.0),
        ]);

        let got = suggest(&library[0], &library, &features, 10);
        let quiet = got
            .iter()
            .find(|s| s.track.id == "same-artist-quiet")
            .unwrap();
        assert!(quiet.reason.contains("similar sound"), "{}", quiet.reason);
        // both signals are used: the tag-similar quiet track still shows up
        assert!(got.len() == 2);
    }

    #[test]
    fn limit_is_respected_and_seed_is_excluded() {
        let library: Vec<Track> = (0..6)
            .map(|i| track(&format!("t{i}"), "A", Some("rock")))
            .collect();
        let got = suggest(&library[0], &library, &HashMap::new(), 3);
        assert_eq!(got.len(), 3);
        assert!(got.iter().all(|s| s.track.id != "t0"));
    }

    #[test]
    fn sound_similarity_is_one_for_identical_tracks() {
        let f = AudioFeatures {
            energy: 0.5,
            bpm: 120.0,
            beat_strength: 0.3,
        };
        assert_eq!(sound_similarity(&f, &f), 1.0);
    }
}
