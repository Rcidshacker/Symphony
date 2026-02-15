//! Integrations module (Phase 5)
//!
//! Developer workflow integrations:
//! - Git monitoring
//! - Pomodoro timer
//! - Status line output

pub mod git;
pub mod pomodoro;
pub mod statusline;

// Re-export main types
pub use git::{ActivityLevel, GitMonitor};
pub use pomodoro::{PomodoroEvent, PomodoroState, PomodoroTimer};
pub use statusline::StatusLine;
