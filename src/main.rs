//! Symphony v2.0 - AI-Powered Terminal Music Player
//!
//! A sophisticated terminal music player with AI integration,
//! real-time visualizations, and developer-focused features.

mod app;
mod audio;
mod config;
mod db;
mod scanner;
mod ui;
mod ai;
mod streaming;
mod plugins;
mod lyrics;
mod integrations;

use std::io;

use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::app::App;
use crate::audio::AudioEngine;
use crate::config::Config;

/// Main entry point for Symphony
fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Load configuration
    let config = Config::load()?;

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Initialize audio engine
    let audio_engine = AudioEngine::new()?;

    // Create application state
    let mut app = App::new(config, audio_engine);

    // Run the main event loop
    let res = run_event_loop(&mut terminal, &mut app);

    // Restore terminal
    restore_terminal(&mut terminal)?;

    // Report any errors
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
