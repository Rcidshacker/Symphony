//! Album art: covers embedded in tags or stored next to the track become colored ASCII art
//! sized for the album-art panel. Fixtures live in tests/fixtures/audio (made with ffmpeg).

mod common;

use std::collections::HashSet;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use ratatui::{backend::TestBackend, Terminal};
use symphony::app::{App, Track, TrackSource};
use symphony::config::Config;
use symphony::ui::artwork::{extract_album_art, ArtworkConfig, AsciiArt, ART_COLS, ART_ROWS};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/audio")
        .join(name)
}

fn panel_config() -> ArtworkConfig {
    ArtworkConfig {
        max_width: ART_COLS,
        max_height: ART_ROWS,
        ..ArtworkConfig::default()
    }
}

#[test]
fn embedded_covers_are_extracted_from_mp3_and_flac() {
    let png = extract_album_art(&fixture("tone_cover.mp3")).expect("PNG cover in the mp3");
    assert_eq!(&png[..4], b"\x89PNG");

    let jpeg = extract_album_art(&fixture("tone_cover.flac")).expect("JPEG cover in the flac");
    assert_eq!(&jpeg[..2], b"\xff\xd8");
}

#[test]
fn a_cover_file_next_to_the_track_is_used_when_nothing_is_embedded() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::copy(fixture("tone.mp3"), dir.path().join("song.mp3")).unwrap();
    assert!(extract_album_art(&dir.path().join("song.mp3")).is_none());

    std::fs::copy(fixture("cover.jpg"), dir.path().join("cover.jpg")).unwrap();
    let art = extract_album_art(&dir.path().join("song.mp3")).expect("cover.jpg next to the track");
    assert_eq!(&art[..2], b"\xff\xd8");
}

#[test]
fn covers_become_colored_ascii_that_fits_the_panel() {
    for cover in ["cover.png", "cover.jpg"] {
        let bytes = std::fs::read(fixture(cover)).unwrap();
        let art = AsciiArt::from_image(&bytes, &panel_config()).unwrap();

        assert_eq!((art.original_width, art.original_height), (120, 120));
        assert!(art.ascii_width <= ART_COLS && art.ascii_height <= ART_ROWS);
        // a square image is twice as many cells wide as tall (the panel is 18 wide: 18 x 9)
        assert_eq!(art.ascii_width, ART_COLS, "{cover}: width");
        assert_eq!(art.ascii_height, ART_COLS / 2, "{cover}: height");

        let colors: HashSet<_> = art.colors.iter().flatten().flatten().collect();
        assert!(
            colors.len() > 8,
            "{cover}: only {} distinct colors",
            colors.len()
        );
        let chars: HashSet<_> = art.chars.iter().flatten().collect();
        assert!(
            chars.len() > 3,
            "{cover}: only {} distinct characters",
            chars.len()
        );
    }
}

#[test]
fn wide_images_keep_their_shape() {
    let image = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        200,
        50,
        image::Rgba([200, 30, 30, 255]),
    ));
    let mut png = Cursor::new(Vec::new());
    image.write_to(&mut png, image::ImageFormat::Png).unwrap();

    let art = AsciiArt::from_image(&png.into_inner(), &panel_config()).unwrap();
    assert_eq!(art.ascii_width, ART_COLS);
    assert_eq!(
        art.ascii_height, 2,
        "200x50 at {ART_COLS} columns is 2 rows of cells"
    );
}

#[test]
fn undecodable_data_is_an_error_not_a_blank_cover() {
    assert!(AsciiArt::from_image(b"not an image", &panel_config()).is_err());
}

fn track_for(path: PathBuf) -> Track {
    Track {
        id: path.display().to_string(),
        title: "Tone".into(),
        artist: "Test".into(),
        album: "Test".into(),
        duration: 1.5,
        path,
        track_number: None,
        genre: None,
        year: None,
        source: TrackSource::Local,
    }
}

fn play_muted(path: PathBuf) -> Option<App> {
    let engine = common::audio_engine()?;
    let mut app = App::new(Config::default(), engine);
    app.audio_engine.set_volume(0.0);
    app.library.tracks = vec![track_for(path)];
    app.select_current_track();
    assert!(
        app.playback_state.is_playing,
        "{:?}",
        app.ui_state.error_message
    );
    Some(app)
}

/// Text of the album-art panel (left 20 columns of the top section)
fn art_panel(app: &App) -> String {
    let mut terminal = Terminal::new(TestBackend::new(120, 32)).unwrap();
    terminal.draw(|f| symphony::ui::render(f, app)).unwrap();
    let buffer = terminal.backend().buffer().clone();
    (4..14u16)
        .map(|y| {
            (1..19u16)
                .map(|x| buffer[(x, y)].symbol().to_string())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn playing_a_track_with_a_cover_shows_it_in_the_panel() {
    let Some(app) = play_muted(fixture("tone_cover.mp3")) else {
        return;
    };
    assert!(app.current_track_artwork.is_some(), "cover was not loaded");

    let panel = art_panel(&app);
    println!("{panel}");
    assert!(
        !panel.contains('♪'),
        "still showing the music-note placeholder"
    );
    assert!(
        panel.chars().any(|c| "#%@*+=".contains(c)),
        "no cover pixels drawn"
    );
}

#[test]
fn a_track_without_a_cover_falls_back_to_the_placeholder() {
    // a temp dir, so the cover.png/cover.jpg fixtures next to tone.mp3 are not picked up
    let dir = tempfile::tempdir().unwrap();
    std::fs::copy(fixture("tone.mp3"), dir.path().join("plain.mp3")).unwrap();
    let Some(app) = play_muted(dir.path().join("plain.mp3")) else {
        return;
    };
    assert!(app.current_track_artwork.is_none());
    assert!(art_panel(&app).contains('♪'));
}
