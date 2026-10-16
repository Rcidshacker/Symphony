//! Keyboard input handling.
//!
//! Lives in the library (not `main.rs`) so it can be tested without a terminal.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::app::App;

/// Handle one key event. Returns `Ok(false)` when the app should quit.
pub fn handle_key_event(app: &mut App, key: KeyEvent) -> anyhow::Result<bool> {
    // Windows reports a press and a release for every key; acting on both
    // toggled things (shuffle, pause, ...) twice.
    if key.kind == KeyEventKind::Release {
        return Ok(true);
    }

    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(KeyModifiers::ALT);

    if ctrl && key.code == KeyCode::Char('c') {
        return Ok(false);
    }

    // The search prompt takes every key as text, so letters bound to
    // commands (q, n, s, ...) can be typed.
    if app.is_in_search_mode() {
        handle_text_input(app, key.code, ctrl || alt);
        return Ok(true);
    }

    if ctrl || alt {
        return Ok(true);
    }

    // Shift is ignored for characters: `+` and `?` arrive with SHIFT set.
    match key.code {
        KeyCode::Char('q') => return Ok(false),

        KeyCode::Char(' ') => app.toggle_playback(),
        KeyCode::Char('n') | KeyCode::Char(']') => app.next_track(),
        KeyCode::Char('p') | KeyCode::Char('[') => app.previous_track(),

        KeyCode::Char('+') | KeyCode::Char('=') => app.volume_up(),
        KeyCode::Char('-') => app.volume_down(),
        KeyCode::Char('m') => app.toggle_mute(),

        KeyCode::Char('s') => app.toggle_shuffle(),
        KeyCode::Char('r') => app.toggle_repeat(),

        KeyCode::Char('/') => app.enter_search_mode(),

        KeyCode::Esc => app.exit_search_mode(),

        KeyCode::Up | KeyCode::Char('k') => app.navigate_up(),
        KeyCode::Down | KeyCode::Char('j') => app.navigate_down(),
        KeyCode::Enter => app.select_current_track(),

        KeyCode::Char('h') | KeyCode::Char('?') => app.toggle_help(),

        KeyCode::Char('v') => app.next_visualization_mode(),
        KeyCode::Char('t') => app.next_theme(),
        KeyCode::Char('1') => app.set_visualization_mode(1),
        KeyCode::Char('2') => app.set_visualization_mode(2),
        KeyCode::Char('3') => app.set_visualization_mode(3),
        KeyCode::Char('4') => app.set_visualization_mode(4),

        _ => {}
    }

    Ok(true)
}

/// Keys while a text prompt (search or AI) is open.
fn handle_text_input(app: &mut App, code: KeyCode, modified: bool) {
    match code {
        KeyCode::Esc => app.exit_search_mode(),
        KeyCode::Enter => app.select_current_track(),
        KeyCode::Backspace => app.remove_search_char(),
        KeyCode::Up => app.navigate_up(),
        KeyCode::Down => app.navigate_down(),
        KeyCode::Char(c) if !modified => app.append_search_char(c),
        _ => {}
    }
}
