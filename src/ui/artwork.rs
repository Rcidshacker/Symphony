//! Album Art Rendering
//!
//! Displays album artwork in the terminal using ASCII art conversion.
//! Supports both embedded metadata and external image files.

use std::path::Path;

use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::Span,
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use thiserror::Error;

use crate::config::theme::Theme;

/// Album art errors
#[derive(Debug, Error)]
pub enum ArtworkError {
    #[error("Failed to read image: {0}")]
    ReadError(String),

    #[error("Failed to decode image: {0}")]
    DecodeError(String),

    #[error("Image too small")]
    TooSmall,

    #[error("No artwork available")]
    NoArtwork,
}

/// Album art configuration
#[derive(Debug, Clone)]
pub struct ArtworkConfig {
    /// Maximum width for ASCII art
    pub max_width: usize,

    /// Maximum height for ASCII art
    pub max_height: usize,

    /// ASCII character set for grayscale mapping (dark to light)
    pub charset: String,

    /// Whether to use color
    pub use_color: bool,

    /// Whether to use inverted colors
    pub invert: bool,

    /// Dithering mode
    pub dither: bool,
}

impl Default for ArtworkConfig {
    fn default() -> Self {
        Self {
            max_width: 40,
            max_height: 20,
            charset: " .:-=+*#%@".to_string(),  // Dark to light
            use_color: true,
            invert: false,
            dither: true,
        }
    }
}

/// ASCII art representation of album artwork
#[derive(Debug, Clone)]
pub struct AsciiArt {
    /// Character grid
    pub chars: Vec<Vec<char>>,

    /// Color grid (optional)
    pub colors: Vec<Vec<Option<Color>>>,

    /// Original dimensions
    pub original_width: u32,
    pub original_height: u32,

    /// ASCII dimensions
    pub ascii_width: usize,
    pub ascii_height: usize,
}

impl AsciiArt {
    /// Create empty ASCII art
    pub fn empty(width: usize, height: usize) -> Self {
        Self {
            chars: vec![vec![' '; width]; height],
            colors: vec![vec![None; width]; height],
            original_width: 0,
            original_height: 0,
            ascii_width: width,
            ascii_height: height,
        }
    }

    /// Create from image data
    pub fn from_image(
        image_data: &[u8],
        config: &ArtworkConfig,
    ) -> Result<Self, ArtworkError> {
        // This is a simplified ASCII art conversion
        // In production, use the `image` crate for proper image processing

        let img = decode_image(image_data)?;

        let (width, height) = img.dimensions();
        if width == 0 || height == 0 {
            return Err(ArtworkError::TooSmall);
        }

        // Calculate output dimensions maintaining aspect ratio
        // Terminal characters are roughly 2x taller than wide
        let aspect_ratio = (width as f64) / (height as f64) * 0.5;
        let target_width = config.max_width.min(width as usize);
        let target_height = (target_width as f64 / aspect_ratio).min(config.max_height as f64) as usize;

        // Resize and convert to ASCII
        let mut chars = Vec::with_capacity(target_height);
        let mut colors = Vec::with_capacity(target_height);

        for y in 0..target_height {
            let mut row = Vec::with_capacity(target_width);
            let mut color_row = Vec::with_capacity(target_width);

            for x in 0..target_width {
                let src_x = (x as f64 * width as f64 / target_width as f64) as u32;
                let src_y = (y as f64 * height as f64 / target_height as f64) as u32;

                if let Some(pixel) = img.get_pixel_checked(src_x, src_y) {
                    let (char, color) = pixel_to_ascii(pixel, config);
                    row.push(char);
                    color_row.push(color);
                } else {
                    row.push(' ');
                    color_row.push(None);
                }
            }

            chars.push(row);
            colors.push(color_row);
        }

        Ok(Self {
            chars,
            colors,
            original_width: width,
            original_height: height,
            ascii_width: target_width,
            ascii_height: target_height,
        })
    }

    /// Create from a placeholder (music note icon)
    pub fn placeholder(width: usize, height: usize, theme: &Theme) -> Self {
        let mut chars = vec![vec![' '; width]; height];
        let colors = vec![vec![Some(theme.accent()); width]; height];

        // Draw a simple music note placeholder
        let center_x = width / 2;
        let center_y = height / 2;

        // Simple music note pattern
        let note_pattern = [
            "    ♪    ",
            "   ♫♪♪   ",
            "  ♪♪♪♪♪  ",
            "   ♪♪♪   ",
            "    ♪    ",
        ];

        for (y, line) in note_pattern.iter().enumerate() {
            if center_y + y < height {
                for (x, c) in line.chars().enumerate() {
                    if center_x.saturating_sub(5) + x < width && c != ' ' {
                        chars[center_y + y][center_x.saturating_sub(5) + x] = c;
                    }
                }
            }
        }

        Self {
            chars,
            colors,
            original_width: 0,
            original_height: 0,
            ascii_width: width,
            ascii_height: height,
        }
    }

    /// Render to string (without colors)
    pub fn to_string(&self) -> String {
        self.chars
            .iter()
            .map(|row| row.iter().collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Simple image representation
struct SimpleImage {
    width: u32,
    height: u32,
    pixels: Vec<Pixel>,
}

impl SimpleImage {
    fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    fn get_pixel_checked(&self, x: u32, y: u32) -> Option<&Pixel> {
        if x < self.width && y < self.height {
            self.pixels.get((y * self.width + x) as usize)
        } else {
            None
        }
    }
}

/// Simple pixel representation
struct Pixel {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

impl Pixel {
    fn grayscale(&self) -> f32 {
        // Luminosity method
        (0.299 * self.r as f32 + 0.587 * self.g as f32 + 0.114 * self.b as f32) / 255.0
    }

    fn average_color(&self) -> Color {
        // Simplified color quantization to terminal colors
        let r = self.r;
        let g = self.g;
        let b = self.b;

        // Determine dominant channel
        if r > 200 && g > 200 && b > 200 {
            Color::White
        } else if r > 200 && g > 100 && b < 100 {
            Color::LightRed
        } else if r > 200 && g > 200 && b < 100 {
            Color::LightYellow
        } else if r < 100 && g > 200 && b < 100 {
            Color::LightGreen
        } else if r < 100 && g > 200 && b > 200 {
            Color::LightCyan
        } else if r < 100 && g < 100 && b > 200 {
            Color::LightBlue
        } else if r > 200 && g < 100 && b > 200 {
            Color::LightMagenta
        } else if r < 50 && g < 50 && b < 50 {
            Color::Black
        } else {
            Color::Gray
        }
    }

    fn to_rgb_color(&self) -> Color {
        Color::Rgb(self.r, self.g, self.b)
    }
}

/// Decode image from bytes (simplified)
fn decode_image(data: &[u8]) -> Result<SimpleImage, ArtworkError> {
    // This is a placeholder implementation
    // In production, use the `image` crate:
    // let img = image::load_from_memory(data)
    //     .map_err(|e| ArtworkError::DecodeError(e.to_string()))?;

    // For now, return a placeholder image
    Ok(SimpleImage {
        width: 100,
        height: 100,
        pixels: vec![
            Pixel { r: 50, g: 50, b: 50, a: 255 };
            100 * 100
        ],
    })
}

/// Convert pixel to ASCII character
fn pixel_to_ascii(pixel: &Pixel, config: &ArtworkConfig) -> (char, Option<Color>) {
    let mut gray = pixel.grayscale();

    if config.invert {
        gray = 1.0 - gray;
    }

    // Map grayscale to character
    let charset = &config.charset;
    let char_index = ((1.0 - gray) * (charset.len() - 1) as f32) as usize;
    let char_index = char_index.min(charset.len() - 1);
    let char = charset.chars().nth(char_index).unwrap_or(' ');

    // Get color
    let color = if config.use_color {
        Some(pixel.to_rgb_color())
    } else {
        None
    };

    (char, color)
}

/// Album art cache
pub struct ArtworkCache {
    /// Cached artwork by track ID
    cache: std::collections::HashMap<String, AsciiArt>,

    /// Maximum cache size
    max_size: usize,
}

impl ArtworkCache {
    /// Create new cache
    pub fn new(max_size: usize) -> Self {
        Self {
            cache: std::collections::HashMap::new(),
            max_size,
        }
    }

    /// Get artwork from cache
    pub fn get(&self, track_id: &str) -> Option<&AsciiArt> {
        self.cache.get(track_id)
    }

    /// Add artwork to cache
    pub fn insert(&mut self, track_id: String, artwork: AsciiArt) {
        // Remove oldest entries if cache is full
        if self.cache.len() >= self.max_size {
            // Simple strategy: remove first entry
            if let Some(key) = self.cache.keys().next().cloned() {
                self.cache.remove(&key);
            }
        }

        self.cache.insert(track_id, artwork);
    }

    /// Clear cache
    pub fn clear(&mut self) {
        self.cache.clear();
    }
}

impl Default for ArtworkCache {
    fn default() -> Self {
        Self::new(50)
    }
}

/// Render album art to frame
pub fn render_album_art(
    f: &mut Frame,
    area: Rect,
    artwork: Option<&AsciiArt>,
    theme: &Theme,
    title: Option<&str>,
) {
    let block_title = title.map(|t| format!(" 🖼️ {} ", t)).unwrap_or_else(|| " 🖼️ ALBUM ART ".to_string());

    let block = Block::default()
        .title(block_title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.width < 5 || inner.height < 3 {
        return;
    }

    match artwork {
        Some(art) => {
            // Render ASCII art
            render_ascii_art(f, inner, art, theme);
        }
        None => {
            // Show placeholder
            let placeholder = AsciiArt::placeholder(inner.width as usize, inner.height as usize, theme);
            render_ascii_art(f, inner, &placeholder, theme);
        }
    }
}

/// Render ASCII art within area
fn render_ascii_art(f: &mut Frame, area: Rect, art: &AsciiArt, theme: &Theme) {
    let start_y = area.y + (area.height.saturating_sub(art.ascii_height as u16) / 2);
    let start_x = area.x + (area.width.saturating_sub(art.ascii_width as u16) / 2);

    for (y, (row, color_row)) in art.chars.iter().zip(art.colors.iter()).enumerate() {
        let y_pos = start_y + y as u16;
        if y_pos >= area.y + area.height {
            break;
        }

        for (x, (&char, &color)) in row.iter().zip(color_row.iter()).enumerate() {
            let x_pos = start_x + x as u16;
            if x_pos >= area.x + area.width {
                break;
            }

            let style = Style::default()
                .fg(color.unwrap_or(theme.text))
                .bg(theme.background);

            let span = Span::styled(char.to_string(), style);
            f.render_widget(
                Paragraph::new(span),
                Rect::new(x_pos, y_pos, 1, 1),
            );
        }
    }
}

/// Extract album art from audio file metadata
pub fn extract_album_art(_path: &Path) -> Option<Vec<u8>> {
    // This would use the `audiotags` or similar crate to extract embedded artwork
    // For now, return None to indicate no embedded artwork
    None
}

/// Download album art from MusicBrainz
pub async fn fetch_album_art(_artist: &str, _album: &str) -> Option<Vec<u8>> {
    // This would use MusicBrainz API and Cover Art Archive
    // Implementation would go here in Phase 4
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_artwork_config_default() {
        let config = ArtworkConfig::default();
        assert_eq!(config.max_width, 40);
        assert_eq!(config.max_height, 20);
        assert!(!config.charset.is_empty());
    }

    #[test]
    fn test_ascii_art_empty() {
        let art = AsciiArt::empty(10, 5);
        assert_eq!(art.ascii_width, 10);
        assert_eq!(art.ascii_height, 5);
        assert!(art.to_string().contains('\n'));
    }

    #[test]
    fn test_ascii_art_placeholder() {
        let theme = Theme::monokai();
        let art = AsciiArt::placeholder(20, 10, &theme);

        assert_eq!(art.ascii_width, 20);
        assert_eq!(art.ascii_height, 10);
        // Should contain music note characters
        let s = art.to_string();
        assert!(s.contains('♪') || s.contains('♫'));
    }

    #[test]
    fn test_artwork_cache() {
        let mut cache = ArtworkCache::new(2);

        let art1 = AsciiArt::empty(10, 10);
        let art2 = AsciiArt::empty(20, 20);
        let art3 = AsciiArt::empty(30, 30);

        cache.insert("track1".to_string(), art1);
        assert!(cache.get("track1").is_some());

        cache.insert("track2".to_string(), art2);
        cache.insert("track3".to_string(), art3);

        // Cache should have evicted first entry
        assert!(cache.get("track1").is_none());
        assert!(cache.get("track2").is_some());
        assert!(cache.get("track3").is_some());
    }

    #[test]
    fn test_pixel_grayscale() {
        let white = Pixel { r: 255, g: 255, b: 255, a: 255 };
        let black = Pixel { r: 0, g: 0, b: 0, a: 255 };
        let gray = Pixel { r: 128, g: 128, b: 128, a: 255 };

        assert!((white.grayscale() - 1.0).abs() < 0.01);
        assert!((black.grayscale() - 0.0).abs() < 0.01);
        assert!((gray.grayscale() - 0.5).abs() < 0.1);
    }
}
