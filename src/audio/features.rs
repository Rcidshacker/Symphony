//! Audio features: loudness ("energy") and tempo, measured from the audio itself.
//!
//! Most libraries have no genre or mood tags (YouTube rips, recordings), so moods like
//! "energetic" or "calm" are matched against these measurements instead.

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use rodio::{Decoder, Source};
use serde::{Deserialize, Serialize};

use super::symphonia_source::SymphoniaSource;

/// Seconds analyzed per track (from the start): enough for tempo, cheap for big libraries
pub const ANALYZED_SECONDS: f32 = 45.0;

/// Measured properties of a track
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AudioFeatures {
    /// Loudness 0.0 (silent) to 1.0 (very loud): average level mapped from -40..0 dBFS
    pub energy: f32,
    /// Estimated tempo in beats per minute (0.0 when no beat was found)
    pub bpm: f32,
    /// How regular the beat is, 0.0 to 1.0 (steady rhythm is high, ambient is low)
    pub beat_strength: f32,
}

/// Analyze the first [`ANALYZED_SECONDS`] of a file. `None` if it cannot be decoded.
pub fn analyze_file(path: &Path) -> Option<AudioFeatures> {
    let (mono, rate) = decode_mono(path, ANALYZED_SECONDS)?;
    if mono.is_empty() {
        return None;
    }
    Some(analyze_samples(&mono, rate))
}

/// Analyze mono samples in -1.0..=1.0
pub fn analyze_samples(mono: &[f32], sample_rate: u32) -> AudioFeatures {
    let (bpm, beat_strength) = estimate_tempo(mono, sample_rate);
    AudioFeatures {
        energy: energy_of(mono),
        bpm,
        beat_strength,
    }
}

fn energy_of(mono: &[f32]) -> f32 {
    if mono.is_empty() {
        return 0.0;
    }
    let mean_square = mono.iter().map(|s| s * s).sum::<f32>() / mono.len() as f32;
    let dbfs = 10.0 * mean_square.max(1e-12).log10();
    ((dbfs + 40.0) / 40.0).clamp(0.0, 1.0)
}

/// Tempo by autocorrelation of the onset envelope (increases in short-term energy)
fn estimate_tempo(mono: &[f32], sample_rate: u32) -> (f32, f32) {
    // 5 ms hop: ~1% tempo resolution
    let hop = (sample_rate as usize / 200).max(1);
    let frames = mono.len() / hop;
    if frames < 400 {
        return (0.0, 0.0); // under two seconds: no meaningful tempo
    }

    // log energy per frame, then positive differences = onsets
    let envelope: Vec<f32> = mono
        .chunks_exact(hop)
        .map(|c| (1.0 + 1000.0 * c.iter().map(|s| s * s).sum::<f32>() / hop as f32).ln())
        .collect();
    let mut onset: Vec<f32> = envelope.windows(2).map(|w| (w[1] - w[0]).max(0.0)).collect();
    let mean = onset.iter().sum::<f32>() / onset.len() as f32;
    onset.iter_mut().for_each(|o| *o -= mean);

    let autocorr = |lag: usize| -> f32 {
        onset[..onset.len() - lag]
            .iter()
            .zip(&onset[lag..])
            .map(|(a, b)| a * b)
            .sum::<f32>()
    };
    let zero = autocorr(0);
    // Too little energy variation to be rhythm (a steady tone's ripple correlates strongly
    // with itself, so judge the size of the onsets, not just their regularity)
    let onset_spread = (zero / onset.len() as f32).sqrt();
    if onset_spread < 0.05 {
        return (0.0, 0.0);
    }

    // 60..180 BPM (lag in hops: 60 / bpm / hop_seconds), with a prior favouring 120 BPM
    let hop_seconds = hop as f32 / sample_rate as f32;
    let lag_of = |bpm: f32| (60.0 / bpm / hop_seconds).round() as usize;
    let (min_lag, max_lag) = (lag_of(180.0), lag_of(60.0).min(onset.len() / 2));

    let mut best = (0usize, f32::MIN, 0.0f32);
    for lag in min_lag..=max_lag {
        let r = autocorr(lag) / zero;
        let bpm = 60.0 / (lag as f32 * hop_seconds);
        let octaves_from_120 = (bpm / 120.0).log2();
        let weighted = r * (-0.5 * (octaves_from_120 / 0.6).powi(2)).exp();
        if weighted > best.1 {
            best = (lag, weighted, r);
        }
    }

    let (lag, _, strength) = best;
    if lag == 0 || strength < 0.05 {
        return (0.0, strength.max(0.0));
    }
    (60.0 / (lag as f32 * hop_seconds), strength.clamp(0.0, 1.0))
}

/// Decode up to `max_secs` of a file to mono floats
fn decode_mono(path: &Path, max_secs: f32) -> Option<(Vec<f32>, u32)> {
    let is_mp4 = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| matches!(e.to_lowercase().as_str(), "m4a" | "mp4"));

    if is_mp4 {
        Some(collect_mono(SymphoniaSource::open(path).ok()?, max_secs))
    } else {
        let decoder = Decoder::new(BufReader::new(File::open(path).ok()?)).ok()?;
        Some(collect_mono(decoder, max_secs))
    }
}

fn collect_mono<S: Source<Item = i16>>(source: S, max_secs: f32) -> (Vec<f32>, u32) {
    let channels = usize::from(source.channels().max(1));
    let rate = source.sample_rate();
    let max_frames = (rate as f32 * max_secs) as usize;

    let mut mono = Vec::with_capacity(max_frames.min(1 << 21));
    let mut frame_sum = 0.0f32;
    let mut in_frame = 0;
    for sample in source {
        frame_sum += f32::from(sample) / 32_768.0;
        in_frame += 1;
        if in_frame == channels {
            mono.push(frame_sum / channels as f32);
            frame_sum = 0.0;
            in_frame = 0;
            if mono.len() >= max_frames {
                break;
            }
        }
    }
    (mono, rate)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 22_050;

    /// A click (30 ms decaying 1 kHz burst) every `60 / bpm` seconds
    fn click_track(bpm: f32, secs: f32) -> Vec<f32> {
        let total = (RATE as f32 * secs) as usize;
        let beat = (RATE as f32 * 60.0 / bpm) as usize;
        let click = (RATE as f32 * 0.03) as usize;
        (0..total)
            .map(|i| {
                let into_beat = i % beat;
                if into_beat < click {
                    let decay = 1.0 - into_beat as f32 / click as f32;
                    (2.0 * std::f32::consts::PI * 1000.0 * into_beat as f32 / RATE as f32).sin() * decay * 0.8
                } else {
                    0.0
                }
            })
            .collect()
    }

    #[test]
    fn finds_the_tempo_of_a_click_track() {
        for bpm in [75.0, 100.0, 120.0, 140.0, 160.0] {
            let features = analyze_samples(&click_track(bpm, 20.0), RATE);
            assert!(
                (features.bpm - bpm).abs() <= bpm * 0.03,
                "{bpm} bpm track measured as {}",
                features.bpm
            );
            assert!(features.beat_strength > 0.3, "{bpm}: strength {}", features.beat_strength);
        }
    }

    #[test]
    fn silence_and_steady_tones_have_no_beat() {
        let silence = analyze_samples(&vec![0.0; RATE as usize * 10], RATE);
        assert_eq!((silence.bpm, silence.energy), (0.0, 0.0));

        let tone: Vec<f32> = (0..RATE as usize * 10)
            .map(|i| (2.0 * std::f32::consts::PI * 220.0 * i as f32 / RATE as f32).sin() * 0.3)
            .collect();
        let steady = analyze_samples(&tone, RATE);
        assert!(steady.beat_strength < 0.1, "steady tone strength {}", steady.beat_strength);
    }

    #[test]
    fn energy_follows_loudness() {
        let tone = |amplitude: f32| -> Vec<f32> {
            (0..RATE as usize * 5)
                .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / RATE as f32).sin() * amplitude)
                .collect()
        };
        let quiet = analyze_samples(&tone(0.02), RATE).energy; // about -37 dBFS rms
        let medium = analyze_samples(&tone(0.2), RATE).energy; // about -17 dBFS
        let loud = analyze_samples(&tone(0.9), RATE).energy; // about -3 dBFS
        assert!(quiet < medium && medium < loud, "{quiet} {medium} {loud}");
        assert!(quiet < 0.15 && loud > 0.85);
    }

    #[test]
    fn very_short_audio_gets_no_tempo() {
        assert_eq!(analyze_samples(&click_track(120.0, 1.0), RATE).bpm, 0.0);
    }
}
