//! Symphony v2.0 - AI-Powered Terminal Music Player
//!
//! A sophisticated terminal music player with AI integration,
//! real-time visualizations, and developer-focused features.

use std::io;
use std::path::Path;

use anyhow::{bail, Context};

use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use symphony::ai::AIManager;
use symphony::app::App;
use symphony::audio::AudioEngine;
use symphony::cli::{self, Command};
use symphony::config::Config;
use symphony::db::Database;
use symphony::scanner::LibraryScanner;
use symphony::streaming::{StreamManager, StreamManagerConfig};
use symphony::ui;

/// Main entry point for Symphony
fn main() -> anyhow::Result<()> {
    let cli = match cli::parse(std::env::args().skip(1)) {
        Ok(cli) => cli,
        Err(err) => {
            eprintln!("error: {err}\n\n{}", cli::USAGE);
            std::process::exit(2);
        }
    };

    // Logs go to stderr: stdout belongs to the TUI and to command output.
    tracing_subscriber::fmt()
        .with_writer(io::stderr)
        .with_max_level(if cli.verbose {
            tracing::Level::DEBUG
        } else {
            tracing::Level::WARN
        })
        .init();

    match cli.command {
        Command::Help => {
            println!("{}", cli::USAGE);
            Ok(())
        }
        Command::Version => {
            println!("symphony {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Command::Init => cmd_init(),
        Command::Scan(dir) => cmd_scan(&dir),
        Command::Add(spec) => cmd_add(&spec),
        Command::Stream(query) => cmd_stream(&query),
        Command::Ask(prompt) => cmd_ask(&prompt),
        Command::Tui => run_tui(Config::load()?),
    }
}

/// Open the library database, creating its directory if needed.
fn open_database(config: &Config) -> anyhow::Result<Database> {
    let path = &config.general.database_path;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let path = path.to_str().context("database path is not valid UTF-8")?;
    Ok(Database::new(path)?)
}

/// `symphony init`: create config (if missing), data directories and database.
fn cmd_init() -> anyhow::Result<()> {
    let config = Config::load()?; // writes the default config on first run
    std::fs::create_dir_all(&config.general.cache_directory)?;
    open_database(&config)?;

    println!("Config:   {}", Config::default_config_path().display());
    println!("Database: {}", config.general.database_path.display());
    println!("Cache:    {}", config.general.cache_directory.display());
    println!("Next: symphony scan <music-dir>");
    Ok(())
}

/// `symphony scan <dir>`: add a directory's music to the library.
fn cmd_scan(dir: &Path) -> anyhow::Result<()> {
    if !dir.exists() {
        bail!("path does not exist: {}", dir.display());
    }
    let config = Config::load()?;
    let db = open_database(&config)?;

    let tracks = LibraryScanner::new().scan_directory(dir)?;
    for track in &tracks {
        db.upsert_track(track)?;
    }
    println!(
        "Scanned {}: {} tracks added or updated, {} in library",
        dir.display(),
        tracks.len(),
        db.track_count()?
    );
    Ok(())
}

/// `symphony add <path>`: local paths only; stream sources are not wired in yet.
fn cmd_add(spec: &str) -> anyhow::Result<()> {
    if spec.starts_with("youtube:") || spec.starts_with("spotify:") {
        bail!("adding stream sources to the library is not implemented yet; use `symphony stream <query>` to search");
    }
    cmd_scan(Path::new(spec))
}

/// `symphony stream <query>`: search the streaming sources and list results.
fn cmd_stream(query: &str) -> anyhow::Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async {
        let manager = StreamManager::new(StreamManagerConfig::default()).await?;
        let result = manager.search_all(query, 10).await?;
        if result.is_empty() {
            println!("No results for \"{query}\"");
        }
        for (i, track) in result.tracks.iter().enumerate() {
            println!(
                "{:>2}. [{:?}] {} - {} ({}:{:02})",
                i + 1,
                track.source,
                track.artist,
                track.title,
                track.duration / 60,
                track.duration % 60
            );
        }
        Ok(())
    })
}

/// `symphony "<prompt>"`: parse a natural-language command with the AI provider.
fn cmd_ask(prompt: &str) -> anyhow::Result<()> {
    let config = Config::load()?;
    if !config.ai.enabled {
        bail!(
            "AI is disabled; set `enabled = true` under [ai] in {}",
            Config::default_config_path().display()
        );
    }
    let provider = config.ai.get_provider_config()?;

    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async {
        let manager = AIManager::new(provider).await?;
        let intent = manager.parse_command(prompt).await?;
        println!("{intent:#?}");
        Ok(())
    })
}

/// Start the interactive player.
fn run_tui(config: Config) -> anyhow::Result<()> {
    // Fallible setup happens before raw mode so errors print normally.
    let database = open_database(&config)?;
    let audio_engine = AudioEngine::new()?;

    let mut app = App::new(config, audio_engine);
    app.database = Some(database);
    app.load_library()?;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res = run_event_loop(&mut terminal, &mut app);

    restore_terminal(&mut terminal)?;

    if let Err(err) = res {
        eprintln!("Error: {err:?}");
        return Err(err);
    }

    Ok(())
}

/// Main event loop
fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> anyhow::Result<()> {
    loop {
        // Draw the UI
        terminal.draw(|f| ui::render(f, app))?;

        // Handle events
        if event::poll(std::time::Duration::from_millis(16))? {
            // ~60 FPS
            if let Event::Key(key) = event::read()? {
                if !handle_key_event(app, key)? {
                    break;
                }
            }
        }

        // Update app state
        app.update();
    }

    Ok(())
}

/// Handle keyboard input
fn handle_key_event(app: &mut App, key: KeyEvent) -> anyhow::Result<bool> {
    match (key.modifiers, key.code) {
        // Quit
        (KeyModifiers::CONTROL, KeyCode::Char('c')) | (KeyModifiers::NONE, KeyCode::Char('q')) => {
            return Ok(false);
        }

        // Play/Pause
        (KeyModifiers::NONE, KeyCode::Char(' ')) => {
            app.toggle_playback();
        }

        // Next track
        (KeyModifiers::NONE, KeyCode::Char('n')) | (KeyModifiers::NONE, KeyCode::Char(']')) => {
            app.next_track();
        }

        // Previous track
        (KeyModifiers::NONE, KeyCode::Char('p')) | (KeyModifiers::NONE, KeyCode::Char('[')) => {
            app.previous_track();
        }

        // Volume up
        (KeyModifiers::NONE, KeyCode::Char('+')) | (KeyModifiers::NONE, KeyCode::Char('=')) => {
            app.volume_up();
        }

        // Volume down
        (KeyModifiers::NONE, KeyCode::Char('-')) => {
            app.volume_down();
        }

        // Mute toggle
        (KeyModifiers::NONE, KeyCode::Char('m')) => {
            app.toggle_mute();
        }

        // Shuffle toggle
        (KeyModifiers::NONE, KeyCode::Char('s')) => {
            app.toggle_shuffle();
        }

        // Repeat toggle
        (KeyModifiers::NONE, KeyCode::Char('r')) => {
            app.toggle_repeat();
        }

        // Search mode
        (KeyModifiers::NONE, KeyCode::Char('/')) => {
            app.enter_search_mode();
        }

        // Escape - exit search or cancel
        (KeyModifiers::NONE, KeyCode::Esc) => {
            app.exit_search_mode();
        }

        // Navigate playlist
        (KeyModifiers::NONE, KeyCode::Up) | (KeyModifiers::NONE, KeyCode::Char('k')) => {
            app.navigate_up();
        }

        (KeyModifiers::NONE, KeyCode::Down) | (KeyModifiers::NONE, KeyCode::Char('j')) => {
            app.navigate_down();
        }

        // Enter - select/confirm
        (KeyModifiers::NONE, KeyCode::Enter) => {
            app.select_current_track();
        }

        // Help
        (KeyModifiers::NONE, KeyCode::Char('h')) | (KeyModifiers::NONE, KeyCode::Char('?')) => {
            app.toggle_help();
        }

        // Visualization mode toggle
        (KeyModifiers::NONE, KeyCode::Char('v')) => {
            app.next_visualization_mode();
        }

        // Theme cycling
        (KeyModifiers::NONE, KeyCode::Char('t')) => {
            app.next_theme();
        }

        // Direct visualizer mode selection (1-4)
        (KeyModifiers::NONE, KeyCode::Char('1')) => {
            app.set_visualization_mode(1);
        }
        (KeyModifiers::NONE, KeyCode::Char('2')) => {
            app.set_visualization_mode(2);
        }
        (KeyModifiers::NONE, KeyCode::Char('3')) => {
            app.set_visualization_mode(3);
        }
        (KeyModifiers::NONE, KeyCode::Char('4')) => {
            app.set_visualization_mode(4);
        }

        // Handle text input in search mode
        (KeyModifiers::NONE, KeyCode::Char(c)) if app.is_in_search_mode() => {
            app.append_search_char(c);
        }

        (KeyModifiers::NONE, KeyCode::Backspace) if app.is_in_search_mode() => {
            app.remove_search_char();
        }

        _ => {}
    }

    Ok(true)
}

/// Restore terminal to original state
fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> anyhow::Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}
