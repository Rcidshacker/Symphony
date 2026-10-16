//! Symphony v2.0 - AI-Powered Terminal Music Player
//!
//! A sophisticated terminal music player with AI integration,
//! real-time visualizations, and developer-focused features.

use std::io;
use std::path::Path;

use anyhow::{bail, Context};

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use symphony::app::{import_stream_spec, App};
use symphony::audio::AudioEngine;
use symphony::cli::{self, Command};
use symphony::config::Config;
use symphony::db::Database;
use symphony::input::handle_key_event;
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
        Command::Stream(query) => run_tui(Config::load()?, Startup::Stream(&query)),
        Command::Ask(prompt) => run_tui(Config::load()?, Startup::Prompt(&prompt)),
        Command::Tui => run_tui(Config::load()?, Startup::Nothing),
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

/// `symphony add <path | youtube:… | spotify:…>`: add local music or stream tracks to the library.
fn cmd_add(spec: &str) -> anyhow::Result<()> {
    let is_stream = spec.starts_with("youtube:")
        || spec.starts_with("spotify:")
        || spec.starts_with("http://")
        || spec.starts_with("https://");
    if !is_stream {
        return cmd_scan(Path::new(spec));
    }

    let config = Config::load()?;
    let db = open_database(&config)?;
    let runtime = tokio::runtime::Runtime::new()?;
    let added = runtime.block_on(async {
        let manager = StreamManager::new(StreamManagerConfig::from_config(&config.streaming))
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        import_stream_spec(&manager, &db, spec)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))
    })?;
    for track in &added {
        println!("added {} - {} ({})", track.artist, track.title, track.id);
    }
    println!("{} track(s) added, {} in library", added.len(), db.track_count()?);
    Ok(())
}

/// What the player does right after it starts
enum Startup<'a> {
    Nothing,
    /// Run a natural-language prompt (`symphony "play something calm"`)
    Prompt(&'a str),
    /// Search the streaming sources and play the best match (`symphony stream <query>`)
    Stream(&'a str),
}

/// Start the interactive player.
fn run_tui(config: Config, startup: Startup) -> anyhow::Result<()> {
    // Fallible setup happens before raw mode so errors print normally.
    let database = open_database(&config)?;
    let audio_engine = AudioEngine::new()?;

    let mut app = App::new(config, audio_engine);
    app.database = Some(database);
    app.load_library()?;
    match startup {
        Startup::Nothing => {}
        Startup::Prompt(prompt) => app.submit_prompt(prompt),
        Startup::Stream(query) => app.play_stream_query(query),
    }

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
