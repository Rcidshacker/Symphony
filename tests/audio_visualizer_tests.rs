//! End-to-end: audio decoded and played by the real engine reaches the visualizer.
//! Playback is muted (the tap sits before the volume stage). Skipped without an audio device.

mod common;

use std::f32::consts::PI;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use ratatui::{backend::TestBackend, Terminal};
use symphony::app::{App, Track, TrackSource};
use symphony::config::Config;
use symphony::ui::visualizer::VisualizationMode;

/// Write a 16-bit mono 44.1 kHz WAV containing the sum of sine tones
fn write_chord_wav(path: &Path, freqs: &[f32], secs: f32, amplitude: f32) {
    let rate = 44_100u32;
    let samples: Vec<i16> = (0..(rate as f32 * secs) as usize)
        .map(|i| {
            let t = i as f32 / rate as f32;
            let sum: f32 = freqs.iter().map(|f| (2.0 * PI * f * t).sin()).sum();
            (sum / freqs.len() as f32 * amplitude * 32_767.0) as i16
        })
        .collect();
    let data_len = (samples.len() * 2) as u32;

    let mut wav = Vec::new();
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_len).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
    wav.extend_from_slice(&1u16.to_le_bytes()); // mono
    wav.extend_from_slice(&rate.to_le_bytes());
    wav.extend_from_slice(&(rate * 2).to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        wav.extend_from_slice(&s.to_le_bytes());
    }
    std::fs::write(path, wav).unwrap();
}

fn track_for(path: PathBuf) -> Track {
    Track {
        id: "tone".into(),
        title: "Tone".into(),
        artist: "Test".into(),
        album: "Test".into(),
        duration: 3.0,
        path,
        track_number: None,
        genre: None,
        year: None,
        source: TrackSource::Local,
    }
}

/// App playing a muted 1 kHz tone, or `None` when there is no audio device
fn playing_tone_app(dir: &Path) -> Option<App> {
    playing_app(dir, &[1000.0], 0.8)
}

/// App playing a muted chord, or `None` when there is no audio device
fn playing_app(dir: &Path, freqs: &[f32], amplitude: f32) -> Option<App> {
    let engine = common::audio_engine()?;
    let mut app = App::new(Config::default(), engine);
    let wav = dir.join("tone.wav");
    write_chord_wav(&wav, freqs, 3.0, amplitude);

    app.audio_engine.set_volume(0.0);
    app.library.tracks = vec![track_for(wav)];
    app.select_current_track();
    assert!(
        app.playback_state.is_playing,
        "playback did not start: {:?}",
        app.ui_state.error_message
    );
    app.audio_engine.set_volume(0.0); // play_file re-applies the stored volume
    Some(app)
}

fn run_until(app: &mut App, secs: f32, mut done: impl FnMut(&App) -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed().as_secs_f32() < secs {
        app.update();
        if done(app) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(16));
    }
    false
}

#[test]
fn spectrum_follows_the_audio_that_is_playing() {
    let dir = tempfile::tempdir().unwrap();
    let Some(mut app) = playing_tone_app(dir.path()) else {
        return;
    };

    assert!(
        run_until(&mut app, 3.0, |a| a.visualizer_state.spectrum.is_some()),
        "no spectrum arrived from the playing audio"
    );
    // let the bars settle: they rise a step per frame, so a slow CI machine needs longer
    run_until(&mut app, 3.0, |a| {
        a.visualizer_state.bar_heights.iter().any(|h| *h > 0.6)
    });

    let spectrum = app.visualizer_state.spectrum.clone().unwrap();
    assert!(
        (spectrum.peak_frequency - 1000.0).abs() < 25.0,
        "peak was {} Hz",
        spectrum.peak_frequency
    );
    assert!(spectrum.rms > 0.4, "rms was {}", spectrum.rms);
    assert!(
        spectrum.waveform.iter().any(|s| *s > 0.5) && spectrum.waveform.iter().any(|s| *s < -0.5)
    );

    // log-spaced bars: the bar holding 1 kHz is lit, the lowest bar is not
    let bars = &app.visualizer_state.bar_heights;
    let loudest = bars
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .map(|(i, _)| i)
        .unwrap();
    let f_lo: f32 = 40.0;
    let f_hi: f32 = 16_000.0;
    let expected = ((1000.0_f32 / f_lo).ln() / (f_hi / f_lo).ln() * bars.len() as f32) as usize;
    assert!(
        loudest.abs_diff(expected) <= 1,
        "loudest bar {loudest}, expected about {expected} ({bars:?})"
    );
    assert!(bars[loudest] > 0.6);
    assert!(bars[0] < 0.2, "lowest bar was {}", bars[0]);
}

#[test]
fn visualizer_fades_out_when_paused() {
    let dir = tempfile::tempdir().unwrap();
    let Some(mut app) = playing_tone_app(dir.path()) else {
        return;
    };
    assert!(run_until(&mut app, 3.0, |a| a
        .visualizer_state
        .spectrum
        .is_some()));

    app.toggle_playback(); // pause: the audio device stops pulling samples
    assert!(
        run_until(&mut app, 4.0, |a| a.visualizer_state.spectrum.is_none()),
        "visualizer kept showing audio after pause"
    );
    assert!(app.visualizer_state.bar_heights.iter().all(|h| *h == 0.0));
}

#[test]
fn every_visualizer_mode_draws_the_playing_audio() {
    let dir = tempfile::tempdir().unwrap();
    let Some(mut app) = playing_app(dir.path(), &[120.0, 440.0, 1500.0, 4000.0, 9000.0], 0.9)
    else {
        return;
    };
    assert!(run_until(&mut app, 3.0, |a| a
        .visualizer_state
        .spectrum
        .is_some()));
    run_until(&mut app, 3.0, |a| {
        a.visualizer_state.bar_heights.iter().any(|h| *h > 0.3)
    });

    for mode in [
        VisualizationMode::Bars,
        VisualizationMode::Waveform,
        VisualizationMode::Circular,
        VisualizationMode::Minimal,
    ] {
        app.visualizer_state.set_mode(mode);
        let mut terminal = Terminal::new(TestBackend::new(120, 32)).unwrap();
        terminal.draw(|f| symphony::ui::render(f, &app)).unwrap();
        let buffer = terminal.backend().buffer().clone();

        // the spectrum panel is the right-hand column of the 12-row top section, minus its
        // title row (text there is not the visualization)
        let (x0, x1, y0, y1) = (88u16, 120u16, 4u16, 15u16);
        let mut text = String::new();
        let mut drawn = 0;
        for y in y0..y1 {
            for x in x0..x1 {
                let symbol = buffer[(x, y)].symbol();
                text.push_str(symbol);
                if !matches!(symbol, " " | "│" | "─" | "┌" | "┐" | "└" | "┘") {
                    drawn += 1;
                }
            }
            text.push('\n');
        }
        println!("--- {mode:?}\n{text}");
        println!("{mode:?} drew {drawn} cells");
        // a silent panel draws none; a slow CI sound card can show only some of the tones
        assert!(drawn > 10, "{mode:?} drew almost nothing ({drawn} cells)");
    }
}
