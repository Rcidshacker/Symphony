//! Headless performance probe: plays one file for N seconds, rendering the full UI to an
//! in-memory 120x40 terminal at 60 fps, and reports frame times.
//! `cargo run --release --example perf_probe -- <audio file> [seconds]`

use std::time::{Duration, Instant};

use ratatui::{backend::TestBackend, Terminal};
use symphony::app::{App, Track, TrackSource};
use symphony::audio::AudioEngine;
use symphony::config::Config;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let file = args
        .get(1)
        .expect("usage: perf_probe <audio file> [seconds]");
    let secs: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(10);

    let started = Instant::now();
    let mut config = Config::default();
    config.general.cache_directory = std::env::temp_dir().join("symphony-perf-probe");
    config.lyrics.auto_fetch = false;
    config.plugins.enabled = false;
    let mut app = App::new(config, AudioEngine::new()?);
    app.audio_engine.set_volume(0.0);
    app.library.tracks.push(Track {
        id: "probe".into(),
        title: "probe".into(),
        artist: "probe".into(),
        album: String::new(),
        duration: 600.0,
        path: file.into(),
        track_number: None,
        genre: None,
        year: None,
        source: TrackSource::Local,
    });
    let mut terminal = Terminal::new(TestBackend::new(120, 40))?;
    terminal.draw(|f| symphony::ui::render(f, &app))?;
    println!(
        "startup_to_first_frame_ms={}",
        started.elapsed().as_millis()
    );

    app.select_current_track();
    let (mut frames, mut worst, mut total) = (0u32, Duration::ZERO, Duration::ZERO);
    let end = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < end {
        let frame_start = Instant::now();
        app.update();
        terminal.draw(|f| symphony::ui::render(f, &app))?;
        let took = frame_start.elapsed();
        total += took;
        worst = worst.max(took);
        frames += 1;
        std::thread::sleep(Duration::from_millis(16).saturating_sub(took));
    }
    println!(
        "frames={frames} avg_frame_us={} worst_frame_us={}",
        total.as_micros() / u128::from(frames.max(1)),
        worst.as_micros()
    );
    Ok(())
}
