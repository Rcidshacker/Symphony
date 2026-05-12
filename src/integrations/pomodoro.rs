//! Pomodoro Timer
//!
//! Integrated Pomodoro technique timer with break reminders.

use std::time::{Duration, Instant};

/// Pomodoro timer state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PomodoroState {
    /// Timer is idle
    Idle,
    /// Work session in progress
    Working,
    /// Short break in progress
    Break,
    /// Long break in progress (after 4 sessions)
    LongBreak,
    /// Work session completed, waiting to start break
    WorkComplete,
    /// Break completed, waiting to start work
    BreakComplete,
}

impl PomodoroState {
    /// Get display label
    pub fn label(&self) -> &'static str {
        match self {
            PomodoroState::Idle => "Idle",
            PomodoroState::Working => "Working",
            PomodoroState::Break => "Short Break",
            PomodoroState::LongBreak => "Long Break",
            PomodoroState::WorkComplete => "Work Complete!",
            PomodoroState::BreakComplete => "Break Complete!",
        }
    }

    /// Get icon
    pub fn icon(&self) -> &'static str {
        match self {
            PomodoroState::Idle => "⏸️",
            PomodoroState::Working => "🍅",
            PomodoroState::Break => "☕",
            PomodoroState::LongBreak => "🏖️",
            PomodoroState::WorkComplete => "✅",
            PomodoroState::BreakComplete => "🔄",
        }
    }

    /// Check if timer is running
    pub fn is_running(&self) -> bool {
        matches!(
            self,
            PomodoroState::Working | PomodoroState::Break | PomodoroState::LongBreak
        )
    }

    /// Check if in work session
    pub fn is_work(&self) -> bool {
        matches!(self, PomodoroState::Working)
    }

    /// Check if in break
    pub fn is_break(&self) -> bool {
        matches!(self, PomodoroState::Break | PomodoroState::LongBreak)
    }
}

impl std::fmt::Display for PomodoroState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}

/// Pomodoro event
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PomodoroEvent {
    /// Work session completed
    WorkSessionComplete {
        /// Session number (1-4)
        session: u32,
    },
    /// Break completed
    BreakComplete,
    /// Timer started
    Started(PomodoroState),
    /// Timer stopped
    Stopped {
        /// Remaining time in seconds
        remaining_secs: u64,
    },
}

/// Pomodoro timer configuration
#[derive(Debug, Clone)]
pub struct PomodoroConfig {
    /// Work session duration
    pub work_duration: Duration,
    /// Short break duration
    pub break_duration: Duration,
    /// Long break duration
    pub long_break_duration: Duration,
    /// Number of work sessions before long break
    pub sessions_before_long_break: u32,
    /// Auto-start break after work session
    pub auto_start_break: bool,
    /// Auto-start work after break
    pub auto_start_work: bool,
    /// Show notifications
    pub notifications: bool,
}

impl Default for PomodoroConfig {
    fn default() -> Self {
        Self {
            work_duration: Duration::from_secs(25 * 60),
            break_duration: Duration::from_secs(5 * 60),
            long_break_duration: Duration::from_secs(15 * 60),
            sessions_before_long_break: 4,
            auto_start_break: false,
            auto_start_work: false,
            notifications: true,
        }
    }
}

/// Pomodoro timer
pub struct PomodoroTimer {
    /// Configuration
    config: PomodoroConfig,
    /// Current state
    state: PomodoroState,
    /// Session start time
    session_start: Option<Instant>,
    /// Sessions completed in current cycle
    sessions_completed: u32,
    /// Total sessions completed
    total_sessions: u64,
    /// Total work time
    total_work_time: Duration,
    /// Total break time
    total_break_time: Duration,
}

impl PomodoroTimer {
    /// Create a new pomodoro timer
    pub fn new() -> Self {
        Self {
            config: PomodoroConfig::default(),
            state: PomodoroState::Idle,
            session_start: None,
            sessions_completed: 0,
            total_sessions: 0,
            total_work_time: Duration::ZERO,
            total_break_time: Duration::ZERO,
        }
    }

    /// Create with custom configuration
    pub fn with_config(config: PomodoroConfig) -> Self {
        Self {
            config,
            ..Self::new()
        }
    }

    /// Create with custom durations
    pub fn with_durations(work_mins: u64, break_mins: u64) -> Self {
        let config = PomodoroConfig {
            work_duration: Duration::from_secs(work_mins * 60),
            break_duration: Duration::from_secs(break_mins * 60),
            ..Default::default()
        };
        Self::with_config(config)
    }

    /// Start a work session
    pub fn start_work(&mut self) -> PomodoroEvent {
        self.state = PomodoroState::Working;
        self.session_start = Some(Instant::now());
        PomodoroEvent::Started(PomodoroState::Working)
    }

    /// Start a break
    pub fn start_break(&mut self) -> PomodoroEvent {
        // Every N sessions = long break
        if self.sessions_completed > 0
            && self.sessions_completed % self.config.sessions_before_long_break == 0
        {
            self.state = PomodoroState::LongBreak;
        } else {
            self.state = PomodoroState::Break;
        }
        self.session_start = Some(Instant::now());
        PomodoroEvent::Started(self.state)
    }

    /// Stop the timer
    pub fn stop(&mut self) -> PomodoroEvent {
        let remaining = self.get_remaining_time().unwrap_or(Duration::ZERO);

        self.state = PomodoroState::Idle;
        self.session_start = None;

        PomodoroEvent::Stopped {
            remaining_secs: remaining.as_secs(),
        }
    }

    /// Toggle timer (start/stop)
    pub fn toggle(&mut self) -> PomodoroEvent {
        match self.state {
            PomodoroState::Idle | PomodoroState::WorkComplete | PomodoroState::BreakComplete => {
                self.start_work()
            }
            _ => self.stop(),
        }
    }

    /// Skip to next phase
    pub fn skip(&mut self) -> Option<PomodoroEvent> {
        match self.state {
            PomodoroState::Working => {
                // Skip to break
                self.complete_work_session();
                Some(self.start_break())
            }
            PomodoroState::Break | PomodoroState::LongBreak => {
                // Skip to work
                self.state = PomodoroState::Idle;
                self.session_start = None;
                Some(self.start_work())
            }
            _ => None,
        }
    }

    /// Check for completion (call periodically)
    pub fn check_completion(&mut self) -> Option<PomodoroEvent> {
        let start = self.session_start?;
        let elapsed = start.elapsed();
        let total_duration = self.get_current_duration();

        if elapsed >= total_duration {
            match self.state {
                PomodoroState::Working => {
                    self.complete_work_session();
                    return Some(PomodoroEvent::WorkSessionComplete {
                        session: self.sessions_completed,
                    });
                }
                PomodoroState::Break | PomodoroState::LongBreak => {
                    self.complete_break();
                    return Some(PomodoroEvent::BreakComplete);
                }
                _ => {}
            }
        }

        None
    }

    /// Complete a work session
    fn complete_work_session(&mut self) {
        self.sessions_completed += 1;
        self.total_sessions += 1;
        self.total_work_time += self.config.work_duration;

        if self.config.auto_start_break {
            self.state = PomodoroState::Break;
            self.session_start = Some(Instant::now());
        } else {
            self.state = PomodoroState::WorkComplete;
            self.session_start = None;
        }
    }

    /// Complete a break
    fn complete_break(&mut self) {
        self.total_break_time += self.get_current_duration();

        if self.config.auto_start_work {
            self.state = PomodoroState::Working;
            self.session_start = Some(Instant::now());
        } else {
            self.state = PomodoroState::BreakComplete;
            self.session_start = None;
        }
    }

    /// Get remaining time in current session
    pub fn get_remaining_time(&self) -> Option<Duration> {
        let start = self.session_start?;
        let elapsed = start.elapsed();
        let total = self.get_current_duration();

        Some(total.saturating_sub(elapsed))
    }

    /// Get elapsed time in current session
    pub fn get_elapsed_time(&self) -> Option<Duration> {
        self.session_start.map(|s| s.elapsed())
    }

    /// Get current session duration
    pub fn get_current_duration(&self) -> Duration {
        match self.state {
            PomodoroState::Working => self.config.work_duration,
            PomodoroState::Break => self.config.break_duration,
            PomodoroState::LongBreak => self.config.long_break_duration,
            _ => Duration::ZERO,
        }
    }

    /// Get progress (0.0 to 1.0)
    pub fn get_progress(&self) -> f32 {
        let Some(elapsed) = self.get_elapsed_time() else {
            return 0.0;
        };

        let total = self.get_current_duration();
        if total.is_zero() {
            return 0.0;
        }

        (elapsed.as_secs_f32() / total.as_secs_f32()).min(1.0)
    }

    /// Get current state
    pub fn get_state(&self) -> PomodoroState {
        self.state
    }

    /// Get sessions completed in current cycle
    pub fn get_sessions_completed(&self) -> u32 {
        self.sessions_completed
    }

    /// Get total sessions completed
    pub fn get_total_sessions(&self) -> u64 {
        self.total_sessions
    }

    /// Get total work time
    pub fn get_total_work_time(&self) -> Duration {
        self.total_work_time
    }

    /// Get total break time
    pub fn get_total_break_time(&self) -> Duration {
        self.total_break_time
    }

    /// Reset the timer
    pub fn reset(&mut self) {
        self.state = PomodoroState::Idle;
        self.session_start = None;
        self.sessions_completed = 0;
    }

    /// Format remaining time as string
    pub fn format_remaining(&self) -> String {
        match self.get_remaining_time() {
            Some(remaining) => {
                let mins = remaining.as_secs() / 60;
                let secs = remaining.as_secs() % 60;
                format!("{:02}:{:02}", mins, secs)
            }
            None => match self.state {
                PomodoroState::Idle => "Ready".to_string(),
                PomodoroState::WorkComplete => "Break time!".to_string(),
                PomodoroState::BreakComplete => "Ready to work!".to_string(),
                _ => "--:--".to_string(),
            },
        }
    }

    /// Format progress bar
    pub fn format_progress_bar(&self, width: usize) -> String {
        let progress = self.get_progress();
        let filled = (width as f32 * progress) as usize;
        let empty = width - filled;

        format!("[{}{}]", "█".repeat(filled), "░".repeat(empty))
    }

    /// Get statistics summary
    pub fn get_stats(&self) -> PomodoroStats {
        PomodoroStats {
            state: self.state,
            sessions_completed: self.sessions_completed,
            total_sessions: self.total_sessions,
            total_work_time: self.total_work_time,
            total_break_time: self.total_break_time,
            remaining: self.get_remaining_time(),
        }
    }
}

impl Default for PomodoroTimer {
    fn default() -> Self {
        Self::new()
    }
}

/// Pomodoro statistics snapshot
#[derive(Debug, Clone)]
pub struct PomodoroStats {
    /// Current state
    pub state: PomodoroState,
    /// Sessions in current cycle
    pub sessions_completed: u32,
    /// Total sessions
    pub total_sessions: u64,
    /// Total work time
    pub total_work_time: Duration,
    /// Total break time
    pub total_break_time: Duration,
    /// Remaining time
    pub remaining: Option<Duration>,
}

impl PomodoroStats {
    /// Format work time
    pub fn format_work_time(&self) -> String {
        let hours = self.total_work_time.as_secs() / 3600;
        let mins = (self.total_work_time.as_secs() % 3600) / 60;
        format!("{}h {}m", hours, mins)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pomodoro_creation() {
        let timer = PomodoroTimer::new();
        assert_eq!(timer.get_state(), PomodoroState::Idle);
    }

    #[test]
    fn test_start_work() {
        let mut timer = PomodoroTimer::new();
        let event = timer.start_work();

        assert_eq!(event, PomodoroEvent::Started(PomodoroState::Working));
        assert_eq!(timer.get_state(), PomodoroState::Working);
        assert!(timer.get_remaining_time().is_some());
    }

    #[test]
    fn test_stop() {
        let mut timer = PomodoroTimer::new();
        timer.start_work();
        timer.stop();

        assert_eq!(timer.get_state(), PomodoroState::Idle);
    }

    #[test]
    fn test_toggle() {
        let mut timer = PomodoroTimer::new();

        timer.toggle();
        assert_eq!(timer.get_state(), PomodoroState::Working);

        timer.toggle();
        assert_eq!(timer.get_state(), PomodoroState::Idle);
    }

    #[test]
    fn test_format_remaining() {
        let mut timer = PomodoroTimer::with_durations(25, 5);
        assert_eq!(timer.format_remaining(), "Ready");

        timer.start_work();
        // Should show approximately 25:00 (may vary slightly)
        let formatted = timer.format_remaining();
        assert!(formatted.starts_with("25:") || formatted.starts_with("24:"));
    }

    #[test]
    fn test_progress_bar() {
        let mut timer = PomodoroTimer::new();
        timer.start_work();

        let bar = timer.format_progress_bar(10);
        assert!(bar.starts_with('['));
        assert!(bar.ends_with(']'));
    }

    #[test]
    fn test_state_display() {
        assert_eq!(PomodoroState::Working.to_string(), "Working");
        assert_eq!(PomodoroState::Break.to_string(), "Short Break");
    }
}
