//! Theme System
//!
//! Manages color schemes and visual themes for Symphony.
//! Supports runtime theme switching and custom themes.

use std::collections::HashMap;
use std::path::Path;

use ratatui::style::Color;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Theme loading errors
#[derive(Debug, Error)]
pub enum ThemeError {
    #[error("Failed to read theme file: {0}")]
    ReadError(#[from] std::io::Error),

    #[error("Failed to parse theme: {0}")]
    ParseError(#[from] toml::de::Error),

    #[error("Theme not found: {0}")]
    NotFound(String),

    #[error("Invalid theme format: {0}")]
    InvalidFormat(String),
}

/// Complete theme definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    /// Theme name
    pub name: String,

    /// Theme author
    #[serde(default)]
    pub author: String,

    /// Theme description
    #[serde(default)]
    pub description: String,

    /// UI colors
    #[serde(default)]
    pub ui: UiColors,

    /// Spectrum analyzer colors
    #[serde(default)]
    pub spectrum: SpectrumColors,

    /// Accent colors
    #[serde(default)]
    pub accents: AccentColors,
}

impl Theme {
    /// Get theme file extension
    pub const EXTENSION: &'static str = "toml";

    /// Load theme from file
    pub fn from_file(path: &Path) -> Result<Self, ThemeError> {
        let content = std::fs::read_to_string(path)?;
        let theme: Theme = toml::from_str(&content)?;

        // Validate theme
        if theme.name.is_empty() {
            return Err(ThemeError::InvalidFormat("Theme name is required".to_string()));
        }

        Ok(theme)
    }

    /// Save theme to file
    pub fn to_file(&self, path: &Path) -> Result<(), ThemeError> {
        let content = toml::to_string_pretty(self)
            .map_err(|e| ThemeError::InvalidFormat(e.to_string()))?;
        std::fs::write(path, content)?;
        Ok(())
    }

    // Convenience accessors for flat color access (used by visualizer)
    pub fn background(&self) -> Color {
        self.ui.background
    }

    pub fn foreground(&self) -> Color {
        self.ui.foreground
    }

    pub fn border(&self) -> Color {
        self.ui.border
    }

    pub fn text(&self) -> Color {
        self.ui.text
    }

    pub fn text_dim(&self) -> Color {
        self.ui.text_dim
    }

    pub fn accent(&self) -> Color {
        self.accents.primary
    }

    pub fn success(&self) -> Color {
        self.accents.success
    }

    pub fn error(&self) -> Color {
        self.accents.error
    }

    pub fn warning(&self) -> Color {
        self.accents.warning
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::monokai()
    }
}

/// UI colors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiColors {
    /// Background color
    #[serde(with = "color_serde")]
    pub background: Color,

    /// Foreground (default text) color
    #[serde(with = "color_serde")]
    pub foreground: Color,

    /// Border color
    #[serde(with = "color_serde")]
    pub border: Color,

    /// Primary text color
    #[serde(with = "color_serde")]
    pub text: Color,

    /// Dimmed text color
    #[serde(with = "color_serde")]
    pub text_dim: Color,

    /// Selected item background
    #[serde(with = "color_serde")]
    pub selection_bg: Color,

    /// Selected item text
    #[serde(with = "color_serde")]
    pub selection_fg: Color,
}

impl Default for UiColors {
    fn default() -> Self {
        Self {
            background: Color::Reset,
            foreground: Color::White,
            border: Color::DarkGray,
            text: Color::White,
            text_dim: Color::DarkGray,
            selection_bg: Color::Blue,
            selection_fg: Color::White,
        }
    }
}

/// Spectrum analyzer colors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectrumColors {
    /// Sub-bass (20-60 Hz) - Red
    #[serde(with = "color_serde")]
    pub sub_bass: Color,

    /// Bass (60-200 Hz) - Orange/Red
    #[serde(with = "color_serde")]
    pub bass: Color,

    /// Mid-low (200-500 Hz) - Yellow
    #[serde(with = "color_serde")]
    pub mid_low: Color,

    /// Mid (500-2000 Hz) - Green
    #[serde(with = "color_serde")]
    pub mid: Color,

    /// Mid-high (2000-4000 Hz) - Cyan
    #[serde(with = "color_serde")]
    pub mid_high: Color,

    /// High (4000-8000 Hz) - Blue
    #[serde(with = "color_serde")]
    pub high: Color,

    /// Treble (8000-20000 Hz) - Magenta
    #[serde(with = "color_serde")]
    pub treble: Color,

    /// Beat pulse color
    #[serde(with = "color_serde")]
    pub beat_pulse: Color,
}

impl Default for SpectrumColors {
    fn default() -> Self {
        Self {
            sub_bass: Color::Red,
            bass: Color::LightRed,
            mid_low: Color::Yellow,
            mid: Color::Green,
            mid_high: Color::Cyan,
            high: Color::Blue,
            treble: Color::Magenta,
            beat_pulse: Color::White,
        }
    }
}

/// Accent colors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccentColors {
    /// Primary accent
    #[serde(with = "color_serde")]
    pub primary: Color,

    /// Secondary accent
    #[serde(with = "color_serde")]
    pub secondary: Color,

    /// Success color
    #[serde(with = "color_serde")]
    pub success: Color,

    /// Warning color
    #[serde(with = "color_serde")]
    pub warning: Color,

    /// Error color
    #[serde(with = "color_serde")]
    pub error: Color,

    /// Info color
    #[serde(with = "color_serde")]
    pub info: Color,
}

impl Default for AccentColors {
    fn default() -> Self {
        Self {
            primary: Color::Magenta,
            secondary: Color::Blue,
            success: Color::Green,
            warning: Color::Yellow,
            error: Color::Red,
            info: Color::Cyan,
        }
    }
}

/// Theme manager
pub struct ThemeManager {
    /// Loaded themes
    themes: HashMap<String, Theme>,

    /// Current theme name
    current_theme: String,

    /// Themes directory path
    themes_dir: Option<std::path::PathBuf>,
}

impl ThemeManager {
    /// Create new theme manager with built-in themes
    pub fn new() -> Self {
        let mut themes = HashMap::new();

        // Add built-in themes
        let built_in = Self::built_in_themes();
        for theme in built_in {
            themes.insert(theme.name.clone(), theme);
        }

        Self {
            themes,
            current_theme: "monokai".to_string(),
            themes_dir: None,
        }
    }

    /// Set themes directory and load custom themes
    pub fn set_themes_dir(&mut self, path: std::path::PathBuf) -> Result<(), ThemeError> {
        self.themes_dir = Some(path.clone());
        self.load_custom_themes(&path)
    }

    /// Load custom themes from directory
    fn load_custom_themes(&mut self, dir: &Path) -> Result<(), ThemeError> {
        if !dir.exists() {
            return Ok(());
        }

        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().map(|e| e == "toml").unwrap_or(false) {
                if let Ok(theme) = Theme::from_file(&path) {
                    self.themes.insert(theme.name.clone(), theme);
                }
            }
        }

        Ok(())
    }

    /// Get current theme
    pub fn current(&self) -> &Theme {
        self.themes.get(&self.current_theme).unwrap_or_else(|| {
            self.themes.get("monokai").expect("Default theme should exist")
        })
    }

    /// Get current theme name
    pub fn current_name(&self) -> &str {
        &self.current_theme
    }

    /// Set current theme by name
    pub fn set_theme(&mut self, name: &str) -> Result<(), ThemeError> {
        if self.themes.contains_key(name) {
            self.current_theme = name.to_string();
            Ok(())
        } else {
            Err(ThemeError::NotFound(name.to_string()))
        }
    }

    /// Get list of available themes
    pub fn available_themes(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.themes.keys().map(|s| s.as_str()).collect();
        names.sort();
        names
    }

    /// Get theme by name
    pub fn get(&self, name: &str) -> Option<&Theme> {
        self.themes.get(name)
    }

    /// Get theme at index
    pub fn get_at_index(&self, index: usize) -> Option<&Theme> {
        let themes = self.available_themes();
        themes.get(index).and_then(|name| self.themes.get(*name))
    }

    /// Get current theme index
    pub fn current_index(&self) -> usize {
        let themes = self.available_themes();
        themes.iter()
            .position(|&t| t == self.current_theme)
            .unwrap_or(0)
    }

    /// Switch to next theme
    pub fn next_theme(&mut self) -> &Theme {
        let themes = self.available_themes();
        let current_idx = self.current_index();
        let next_idx = (current_idx + 1) % themes.len();

        if let Some(&next_name) = themes.get(next_idx) {
            self.current_theme = next_name.to_string();
        }

        self.current()
    }

    /// Get built-in themes
    fn built_in_themes() -> Vec<Theme> {
        vec![
            Theme::monokai(),
            Theme::dracula(),
            Theme::nord(),
            Theme::solarized_dark(),
            Theme::gruvbox(),
        ]
    }
}

impl Default for ThemeManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Built-in Theme Definitions
// ============================================================================

impl Theme {
    /// Monokai Pro theme
    pub fn monokai() -> Self {
        Self {
            name: "monokai".to_string(),
            author: "Wimer Hazenberg".to_string(),
            description: "Classic Monokai color scheme".to_string(),
            ui: UiColors {
                background: Color::Rgb(45, 45, 45),
                foreground: Color::Rgb(253, 253, 253),
                border: Color::Rgb(89, 89, 89),
                text: Color::Rgb(253, 253, 253),
                text_dim: Color::Rgb(150, 150, 150),
                selection_bg: Color::Rgb(102, 102, 102),
                selection_fg: Color::Rgb(253, 253, 253),
            },
            spectrum: SpectrumColors {
                sub_bass: Color::Rgb(255, 97, 136),    // Red
                bass: Color::Rgb(255, 140, 100),       // Orange
                mid_low: Color::Rgb(255, 216, 102),    // Yellow
                mid: Color::Rgb(169, 220, 118),        // Green
                mid_high: Color::Rgb(120, 220, 232),   // Cyan
                high: Color::Rgb(171, 157, 242),       // Purple
                treble: Color::Rgb(255, 147, 183),     // Pink
                beat_pulse: Color::Rgb(255, 255, 255),
            },
            accents: AccentColors {
                primary: Color::Rgb(255, 97, 136),     // Red
                secondary: Color::Rgb(171, 157, 242),  // Purple
                success: Color::Rgb(169, 220, 118),    // Green
                warning: Color::Rgb(255, 216, 102),    // Yellow
                error: Color::Rgb(255, 97, 136),       // Red
                info: Color::Rgb(120, 220, 232),       // Cyan
            },
        }
    }

    /// Dracula theme
    pub fn dracula() -> Self {
        Self {
            name: "dracula".to_string(),
            author: "Zeno Rocha".to_string(),
            description: "Dark theme with purple accents".to_string(),
            ui: UiColors {
                background: Color::Rgb(40, 42, 54),
                foreground: Color::Rgb(248, 248, 242),
                border: Color::Rgb(98, 114, 164),
                text: Color::Rgb(248, 248, 242),
                text_dim: Color::Rgb(139, 143, 167),
                selection_bg: Color::Rgb(68, 71, 90),
                selection_fg: Color::Rgb(248, 248, 242),
            },
            spectrum: SpectrumColors {
                sub_bass: Color::Rgb(255, 85, 85),     // Red
                bass: Color::Rgb(255, 121, 198),       // Pink
                mid_low: Color::Rgb(255, 184, 108),    // Orange
                mid: Color::Rgb(241, 250, 140),        // Yellow
                mid_high: Color::Rgb(80, 250, 123),    // Green
                high: Color::Rgb(139, 233, 253),       // Cyan
                treble: Color::Rgb(189, 147, 249),     // Purple
                beat_pulse: Color::Rgb(255, 255, 255),
            },
            accents: AccentColors {
                primary: Color::Rgb(189, 147, 249),    // Purple
                secondary: Color::Rgb(139, 233, 253),  // Cyan
                success: Color::Rgb(80, 250, 123),     // Green
                warning: Color::Rgb(255, 184, 108),    // Orange
                error: Color::Rgb(255, 85, 85),        // Red
                info: Color::Rgb(139, 233, 253),       // Cyan
            },
        }
    }

    /// Nord theme
    pub fn nord() -> Self {
        Self {
            name: "nord".to_string(),
            author: "Arctic Ice Studio".to_string(),
            description: "Arctic, bluish color palette".to_string(),
            ui: UiColors {
                background: Color::Rgb(46, 52, 64),
                foreground: Color::Rgb(236, 239, 244),
                border: Color::Rgb(76, 86, 106),
                text: Color::Rgb(236, 239, 244),
                text_dim: Color::Rgb(143, 188, 187),
                selection_bg: Color::Rgb(94, 129, 172),
                selection_fg: Color::Rgb(236, 239, 244),
            },
            spectrum: SpectrumColors {
                sub_bass: Color::Rgb(191, 97, 106),    // Aurora Red
                bass: Color::Rgb(208, 135, 112),       // Aurora Orange
                mid_low: Color::Rgb(235, 203, 139),    // Aurora Yellow
                mid: Color::Rgb(163, 190, 140),        // Aurora Green
                mid_high: Color::Rgb(136, 192, 208),   // Frost Cyan
                high: Color::Rgb(129, 161, 193),       // Frost Blue
                treble: Color::Rgb(180, 142, 173),     // Aurora Purple
                beat_pulse: Color::Rgb(143, 188, 187),
            },
            accents: AccentColors {
                primary: Color::Rgb(136, 192, 208),    // Frost Cyan
                secondary: Color::Rgb(129, 161, 193),  // Frost Blue
                success: Color::Rgb(163, 190, 140),    // Aurora Green
                warning: Color::Rgb(235, 203, 139),    // Aurora Yellow
                error: Color::Rgb(191, 97, 106),       // Aurora Red
                info: Color::Rgb(143, 188, 187),       // Frost Teal
            },
        }
    }

    /// Solarized Dark theme
    pub fn solarized_dark() -> Self {
        Self {
            name: "solarized-dark".to_string(),
            author: "Ethan Schoonover".to_string(),
            description: "Precision color scheme".to_string(),
            ui: UiColors {
                background: Color::Rgb(7, 54, 66),
                foreground: Color::Rgb(253, 246, 227),
                border: Color::Rgb(0, 43, 54),
                text: Color::Rgb(253, 246, 227),
                text_dim: Color::Rgb(147, 161, 161),
                selection_bg: Color::Rgb(0, 43, 54),
                selection_fg: Color::Rgb(253, 246, 227),
            },
            spectrum: SpectrumColors {
                sub_bass: Color::Rgb(220, 50, 47),     // Red
                bass: Color::Rgb(203, 75, 22),         // Orange
                mid_low: Color::Rgb(181, 137, 0),      // Yellow
                mid: Color::Rgb(133, 153, 0),          // Green
                mid_high: Color::Rgb(42, 161, 152),    // Cyan
                high: Color::Rgb(38, 139, 210),        // Blue
                treble: Color::Rgb(108, 113, 196),     // Violet
                beat_pulse: Color::Rgb(238, 232, 213),
            },
            accents: AccentColors {
                primary: Color::Rgb(38, 139, 210),     // Blue
                secondary: Color::Rgb(42, 161, 152),   // Cyan
                success: Color::Rgb(133, 153, 0),      // Green
                warning: Color::Rgb(181, 137, 0),      // Yellow
                error: Color::Rgb(220, 50, 47),        // Red
                info: Color::Rgb(42, 161, 152),        // Cyan
            },
        }
    }

    /// Gruvbox theme
    pub fn gruvbox() -> Self {
        Self {
            name: "gruvbox".to_string(),
            author: "Mordechai Hadad".to_string(),
            description: "Retro groove color scheme".to_string(),
            ui: UiColors {
                background: Color::Rgb(40, 40, 40),
                foreground: Color::Rgb(235, 219, 178),
                border: Color::Rgb(60, 56, 54),
                text: Color::Rgb(235, 219, 178),
                text_dim: Color::Rgb(146, 131, 116),
                selection_bg: Color::Rgb(80, 73, 69),
                selection_fg: Color::Rgb(235, 219, 178),
            },
            spectrum: SpectrumColors {
                sub_bass: Color::Rgb(251, 73, 52),     // Bright Red
                bass: Color::Rgb(251, 150, 40),        // Bright Orange
                mid_low: Color::Rgb(250, 189, 47),     // Bright Yellow
                mid: Color::Rgb(184, 187, 38),         // Bright Green
                mid_high: Color::Rgb(142, 192, 124),   // Bright Aqua
                high: Color::Rgb(131, 165, 152),       // Bright Blue
                treble: Color::Rgb(211, 134, 155),     // Bright Purple
                beat_pulse: Color::Rgb(251, 241, 199),
            },
            accents: AccentColors {
                primary: Color::Rgb(251, 150, 40),     // Bright Orange
                secondary: Color::Rgb(131, 165, 152),  // Bright Blue
                success: Color::Rgb(184, 187, 38),     // Bright Green
                warning: Color::Rgb(250, 189, 47),     // Bright Yellow
                error: Color::Rgb(251, 73, 52),        // Bright Red
                info: Color::Rgb(142, 192, 124),       // Bright Aqua
            },
        }
    }
}

// ============================================================================
// Color Serialization Helper
// ============================================================================

mod color_serde {
    use ratatui::style::Color;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(color: &Color, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = color_to_string(color);
        serializer.serialize_str(&s)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Color, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        string_to_color(&s).map_err(serde::de::Error::custom)
    }

    fn color_to_string(color: &Color) -> String {
        match color {
            Color::Reset => "reset".to_string(),
            Color::Black => "black".to_string(),
            Color::Red => "red".to_string(),
            Color::Green => "green".to_string(),
            Color::Yellow => "yellow".to_string(),
            Color::Blue => "blue".to_string(),
            Color::Magenta => "magenta".to_string(),
            Color::Cyan => "cyan".to_string(),
            Color::Gray => "gray".to_string(),
            Color::DarkGray => "dark_gray".to_string(),
            Color::LightRed => "light_red".to_string(),
            Color::LightGreen => "light_green".to_string(),
            Color::LightYellow => "light_yellow".to_string(),
            Color::LightBlue => "light_blue".to_string(),
            Color::LightMagenta => "light_magenta".to_string(),
            Color::LightCyan => "light_cyan".to_string(),
            Color::White => "white".to_string(),
            Color::Rgb(r, g, b) => format!("#{:02x}{:02x}{:02x}", r, g, b),
            Color::Indexed(i) => format!("indexed({})", i),
        }
    }

    fn string_to_color(s: &str) -> Result<Color, String> {
        let s = s.trim().to_lowercase();

        match s.as_str() {
            "reset" => Ok(Color::Reset),
            "black" => Ok(Color::Black),
            "red" => Ok(Color::Red),
            "green" => Ok(Color::Green),
            "yellow" => Ok(Color::Yellow),
            "blue" => Ok(Color::Blue),
            "magenta" => Ok(Color::Magenta),
            "cyan" => Ok(Color::Cyan),
            "gray" | "grey" => Ok(Color::Gray),
            "dark_gray" | "dark_grey" => Ok(Color::DarkGray),
            "light_red" => Ok(Color::LightRed),
            "light_green" => Ok(Color::LightGreen),
            "light_yellow" => Ok(Color::LightYellow),
            "light_blue" => Ok(Color::LightBlue),
            "light_magenta" => Ok(Color::LightMagenta),
            "light_cyan" => Ok(Color::LightCyan),
            "white" => Ok(Color::White),
            _ => {
                // Try parsing as hex color
                if s.starts_with('#') && s.len() == 7 {
                    let r = u8::from_str_radix(&s[1..3], 16)
                        .map_err(|e| e.to_string())?;
                    let g = u8::from_str_radix(&s[3..5], 16)
                        .map_err(|e| e.to_string())?;
                    let b = u8::from_str_radix(&s[5..7], 16)
                        .map_err(|e| e.to_string())?;
                    Ok(Color::Rgb(r, g, b))
                } else {
                    Err(format!("Unknown color: {}", s))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_themes() {
        let themes = vec![
            Theme::monokai(),
            Theme::dracula(),
            Theme::nord(),
            Theme::solarized_dark(),
            Theme::gruvbox(),
        ];

        for theme in themes {
            assert!(!theme.name.is_empty());
        }
    }

    #[test]
    fn test_theme_manager() {
        let mut manager = ThemeManager::new();

        assert_eq!(manager.current_name(), "monokai");

        let themes = manager.available_themes();
        assert!(themes.len() >= 5);

        // Test theme switching
        manager.set_theme("dracula").unwrap();
        assert_eq!(manager.current_name(), "dracula");

        // Test next theme
        manager.next_theme();
        assert_ne!(manager.current_name(), "dracula");
    }

    #[test]
    fn test_color_serialization() {
        let color = Color::Rgb(255, 128, 64);
        let serialized = color_serde::color_to_string(&color);
        assert_eq!(serialized, "#ff8040");

        let deserialized = color_serde::string_to_color(&serialized).unwrap();
        assert_eq!(color, deserialized);
    }

    #[test]
    fn test_theme_serialization() {
        let theme = Theme::monokai();
        let serialized = toml::to_string(&theme).unwrap();
        let deserialized: Theme = toml::from_str(&serialized).unwrap();

        assert_eq!(theme.name, deserialized.name);
    }
}
