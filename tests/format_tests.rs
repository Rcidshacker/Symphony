//! Every format the README lists (MP3, FLAC, AAC, OGG, WAV) must scan with a duration
//! and decode to audio. Fixtures are 1.5 s mono 440 Hz tones made with ffmpeg
//! (tone.<ext> in tests/fixtures/audio, with title/artist tags where the format has them).

use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use rodio::{Decoder, Source};
use symphony::audio::AudioEngine;
use symphony::scanner::LibraryScanner;

fn fixture(ext: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/audio")
        .join(format!("tone.{ext}"))
}

const FORMATS: [&str; 6] = ["mp3", "flac", "ogg", "wav", "aac", "m4a"];

/// Formats rodio decodes itself (M4A goes through our own Symphonia source)
const RODIO_FORMATS: [&str; 5] = ["mp3", "flac", "ogg", "wav", "aac"];

#[test]
fn rodio_formats_decode_to_audio() {
    for ext in RODIO_FORMATS {
        let path = fixture(ext);
        let file = File::open(&path).unwrap_or_else(|e| panic!("{ext}: {e}"));
        let decoder = Decoder::new(BufReader::new(file))
            .unwrap_or_else(|e| panic!("{ext}: rodio could not decode: {e}"));

        let rate = decoder.sample_rate();
        let channels = u32::from(decoder.channels());
        let samples: Vec<i16> = decoder.collect();
        let secs = samples.len() as f32 / (rate * channels) as f32;
        assert!((secs - 1.5).abs() < 0.2, "{ext}: decoded {secs} s of audio");

        let loud = samples.iter().filter(|s| s.abs() > 3_000).count();
        assert!(loud > samples.len() / 4, "{ext}: decoded audio is silent");
    }
}

/// The real engine plays each format (muted) and the audio reaches the visualizer tap
#[test]
fn every_format_plays_through_the_engine() {
    let Ok(mut engine) = AudioEngine::new() else { return }; // no audio device
    engine.set_volume(0.0);

    for ext in FORMATS {
        engine
            .play_file(&fixture(ext))
            .unwrap_or_else(|e| panic!("{ext}: engine could not play: {e}"));

        let start = std::time::Instant::now();
        let mut samples: Vec<f32> = Vec::new();
        while samples.len() < 8_000 && start.elapsed().as_secs_f32() < 3.0 {
            std::thread::sleep(std::time::Duration::from_millis(20));
            samples.extend(engine.drain_samples());
        }
        assert!(samples.len() >= 8_000, "{ext}: only {} samples reached the tap", samples.len());

        let rms = (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt();
        assert!(rms > 0.05, "{ext}: playback was silent (rms {rms})");
        assert!(engine.sample_rate() > 0);
        engine.stop();
    }
}

#[test]
fn every_format_is_scanned_with_a_duration() {
    for ext in FORMATS {
        let dir = tempfile::tempdir().unwrap();
        std::fs::copy(fixture(ext), dir.path().join(format!("Some Artist - Some Song.{ext}"))).unwrap();

        let tracks = LibraryScanner::new().scan_directory(dir.path()).unwrap();
        assert_eq!(tracks.len(), 1, "{ext}: the scanner skipped the file");
        assert!(
            (tracks[0].duration - 1.5).abs() < 0.15,
            "{ext}: duration was {}",
            tracks[0].duration
        );
    }
}

#[test]
fn tags_are_read_from_formats_that_carry_them() {
    for ext in ["mp3", "flac", "ogg", "m4a"] {
        let dir = tempfile::tempdir().unwrap();
        std::fs::copy(fixture(ext), dir.path().join(format!("untagged-name.{ext}"))).unwrap();

        let tracks = LibraryScanner::new().scan_directory(dir.path()).unwrap();
        assert_eq!(tracks[0].title, "Fixture Tone", "{ext}: title");
        assert_eq!(tracks[0].artist, "Fixture Artist", "{ext}: artist");
    }
}
