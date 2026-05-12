//! Git Integration
//!
//! Monitors git repository activity to adapt music playback.
//! Can detect "coding sessions" and suggest appropriate music.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};
use tracing::{debug, info, warn};

/// Git activity level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivityLevel {
    /// No recent activity
    Idle,
    /// Light activity (1-2 commits/hour)
    Light,
    /// Focused work (3-5 commits/hour)
    Focused,
    /// Intense coding session (>5 commits/hour)
    Intense,
}

impl ActivityLevel {
    /// Get recommended playlist mood
    pub fn recommended_mood(&self) -> &'static str {
        match self {
            ActivityLevel::Idle => "relaxing",
            ActivityLevel::Light => "chill",
            ActivityLevel::Focused => "focus",
            ActivityLevel::Intense => "energetic",
        }
    }

    /// Get display label
    pub fn label(&self) -> &'static str {
        match self {
            ActivityLevel::Idle => "Idle",
            ActivityLevel::Light => "Light",
            ActivityLevel::Focused => "Focused",
            ActivityLevel::Intense => "Intense",
        }
    }

    /// Get icon
    pub fn icon(&self) -> &'static str {
        match self {
            ActivityLevel::Idle => "💤",
            ActivityLevel::Light => "📝",
            ActivityLevel::Focused => "🔥",
            ActivityLevel::Intense => "⚡",
        }
    }
}

impl std::fmt::Display for ActivityLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}

/// Git monitor configuration
#[derive(Debug, Clone)]
pub struct GitMonitorConfig {
    /// How often to check for updates
    pub check_interval: Duration,
    /// Window for commit velocity calculation
    pub velocity_window: Duration,
    /// Enable auto-adapt music
    pub auto_adapt: bool,
}

impl Default for GitMonitorConfig {
    fn default() -> Self {
        Self {
            check_interval: Duration::from_secs(60),
            velocity_window: Duration::from_secs(3600), // 1 hour
            auto_adapt: true,
        }
    }
}

/// Git repository monitor
pub struct GitMonitor {
    /// Configuration
    config: GitMonitorConfig,
    /// Path to git repository (if found)
    repo_path: Option<PathBuf>,
    /// Last check time
    last_check: Instant,
    /// Commit velocity (commits per hour)
    commit_velocity: f32,
    /// Current activity level
    activity_level: ActivityLevel,
    /// Recent commit timestamps
    recent_commits: Vec<SystemTime>,
    /// Current branch name
    current_branch: Option<String>,
    /// Files changed in last commit
    files_changed: usize,
}

impl GitMonitor {
    /// Create a new git monitor
    pub fn new() -> Self {
        Self {
            config: GitMonitorConfig::default(),
            repo_path: Self::find_git_repo(),
            last_check: Instant::now()
                .checked_sub(Duration::from_secs(3600))
                .unwrap_or_else(Instant::now),
            commit_velocity: 0.0,
            activity_level: ActivityLevel::Idle,
            recent_commits: Vec::new(),
            current_branch: None,
            files_changed: 0,
        }
    }

    /// Create with custom configuration
    pub fn with_config(config: GitMonitorConfig) -> Self {
        Self {
            config,
            ..Self::new()
        }
    }

    /// Find git repository in current directory or parents
    fn find_git_repo() -> Option<PathBuf> {
        let current_dir = std::env::current_dir().ok()?;

        // Walk up the directory tree looking for .git
        let mut path = current_dir.as_path();
        loop {
            let git_dir = path.join(".git");
            if git_dir.exists() {
                return Some(path.to_path_buf());
            }

            path = path.parent()?;
        }
    }

    /// Check if we're in a git repository
    pub fn in_repo(&self) -> bool {
        self.repo_path.is_some()
    }

    /// Get repository path
    pub fn repo_path(&self) -> Option<&Path> {
        self.repo_path.as_deref()
    }

    /// Update git statistics (call periodically)
    pub fn update(&mut self) {
        // Check if enough time has passed
        if self.last_check.elapsed() < self.config.check_interval {
            return;
        }

        let Some(repo_path) = self.repo_path.clone() else {
            return;
        };

        // Try to read git information
        // Note: This is a simplified implementation
        // In production, use the `git2` crate for full functionality

        self.update_from_git_cli(&repo_path);
        self.last_check = Instant::now();
    }

    /// Update using git CLI (fallback when git2 not available)
    fn update_from_git_cli(&mut self, repo_path: &Path) {
        use std::process::Command;

        // Get current branch
        let branch_output = Command::new("git")
            .args(["branch", "--show-current"])
            .current_dir(repo_path)
            .output()
            .ok();

        if let Some(output) = branch_output {
            if output.status.success() {
                self.current_branch =
                    Some(String::from_utf8_lossy(&output.stdout).trim().to_string());
            }
        }

        // Count commits in the last hour
        let since = chrono::Local::now()
            .checked_sub_signed(chrono::Duration::hours(1))
            .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string());

        if let Some(since_str) = since {
            let log_output = Command::new("git")
                .args(["log", "--oneline", "--since", &since_str, "--format=%H"])
                .current_dir(repo_path)
                .output()
                .ok();

            if let Some(output) = log_output {
                if output.status.success() {
                    let count = String::from_utf8_lossy(&output.stdout)
                        .lines()
                        .filter(|l| !l.is_empty())
                        .count();

                    self.commit_velocity = count as f32;

                    // Update activity level
                    self.activity_level = match count {
                        0 => ActivityLevel::Idle,
                        1..=2 => ActivityLevel::Light,
                        3..=5 => ActivityLevel::Focused,
                        _ => ActivityLevel::Intense,
                    };

                    debug!(
                        "Git activity: {} commits/hour -> {:?}",
                        count, self.activity_level
                    );
                }
            }
        }

        // Get files changed in last commit
        let diff_output = Command::new("git")
            .args(["diff", "--name-only", "HEAD~1"])
            .current_dir(repo_path)
            .output()
            .ok();

        if let Some(output) = diff_output {
            if output.status.success() {
                self.files_changed = String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .filter(|l| !l.is_empty())
                    .count();
            }
        }
    }

    /// Get current commit velocity (commits per hour)
    pub fn get_commit_velocity(&self) -> f32 {
        self.commit_velocity
    }

    /// Get current activity level
    pub fn get_activity_level(&self) -> ActivityLevel {
        self.activity_level
    }

    /// Get current branch name
    pub fn get_current_branch(&self) -> Option<&str> {
        self.current_branch.as_deref()
    }

    /// Get files changed in last commit
    pub fn get_files_changed(&self) -> usize {
        self.files_changed
    }

    /// Get statistics summary
    pub fn get_stats(&self) -> GitStats {
        GitStats {
            in_repo: self.repo_path.is_some(),
            branch: self.current_branch.clone(),
            commit_velocity: self.commit_velocity,
            activity_level: self.activity_level,
            files_changed: self.files_changed,
        }
    }

    /// Force refresh (clear cache)
    pub fn force_refresh(&mut self) {
        self.last_check = Instant::now()
            .checked_sub(Duration::from_secs(3600))
            .unwrap_or_else(Instant::now);
        self.update();
    }
}

impl Default for GitMonitor {
    fn default() -> Self {
        Self::new()
    }
}

/// Git statistics snapshot
#[derive(Debug, Clone)]
pub struct GitStats {
    /// In a git repository
    pub in_repo: bool,
    /// Current branch name
    pub branch: Option<String>,
    /// Commit velocity
    pub commit_velocity: f32,
    /// Activity level
    pub activity_level: ActivityLevel,
    /// Files changed in last commit
    pub files_changed: usize,
}

impl GitStats {
    /// Format for display
    pub fn format_summary(&self) -> String {
        if !self.in_repo {
            return "Not in a git repository".to_string();
        }

        let branch = self.branch.as_deref().unwrap_or("unknown");
        format!(
            "{} {} · {} ({:.1} commits/hr)",
            self.activity_level.icon(),
            branch,
            self.activity_level.label(),
            self.commit_velocity
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_activity_level_mood() {
        assert_eq!(ActivityLevel::Idle.recommended_mood(), "relaxing");
        assert_eq!(ActivityLevel::Light.recommended_mood(), "chill");
        assert_eq!(ActivityLevel::Focused.recommended_mood(), "focus");
        assert_eq!(ActivityLevel::Intense.recommended_mood(), "energetic");
    }

    #[test]
    fn test_git_monitor_creation() {
        let monitor = GitMonitor::new();
        // May or may not be in a repo, depends on test environment
    }

    #[test]
    fn test_git_stats_format() {
        let stats = GitStats {
            in_repo: true,
            branch: Some("main".to_string()),
            commit_velocity: 3.5,
            activity_level: ActivityLevel::Focused,
            files_changed: 5,
        };

        let summary = stats.format_summary();
        assert!(summary.contains("main"));
        assert!(summary.contains("Focused"));
    }
}
