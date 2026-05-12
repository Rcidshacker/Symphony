//! Visualizer UI Component
//!
//! Renders audio spectrum analysis in the terminal using Ratatui.
//! Supports multiple visualization modes with smooth animations.

use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::Span,
    widgets::{Block, Borders, Canvas, Paragraph},
    Frame,
};

use crate::audio::analyzer::{FrequencyBand, SpectrumResult};
use crate::config::theme::Theme;

/// Visualization mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum VisualizationMode {
    /// Traditional bar spectrum
    #[default]
    Bars,
    /// Time-domain waveform
    Waveform,
    /// Circular radial visualization
    Circular,
    /// Minimal mode (off)
    Off,
}

impl VisualizationMode {
    /// Get all available modes
    pub fn all() -> &'static [VisualizationMode] {
        &[
            VisualizationMode::Bars,
            VisualizationMode::Waveform,
            VisualizationMode::Circular,
            VisualizationMode::Off,
        ]
    }

    /// Get display name
    pub fn name(&self) -> &'static str {
        match self {
            VisualizationMode::Bars => "Bars",
            VisualizationMode::Waveform => "Waveform",
            VisualizationMode::Circular => "Circular",
            VisualizationMode::Off => "Off",
        }
    }

    /// Cycle to next mode
    pub fn next(&self) -> Self {
        match self {
            VisualizationMode::Bars => VisualizationMode::Waveform,
            VisualizationMode::Waveform => VisualizationMode::Circular,
            VisualizationMode::Circular => VisualizationMode::Off,
            VisualizationMode::Off => VisualizationMode::Bars,
        }
    }
}

/// Frequency band color mapping
pub struct BandColors {
    pub sub_bass: Color,
    pub bass: Color,
    pub mid_low: Color,
    pub mid: Color,
    pub mid_high: Color,
    pub high: Color,
    pub treble: Color,
}

impl BandColors {
    /// Create from theme
    pub fn from_theme(theme: &Theme) -> Self {
        Self {
            sub_bass: theme.spectrum_sub_bass,
            bass: theme.spectrum_bass,
            mid_low: theme.spectrum_mid_low,
            mid: theme.spectrum_mid,
            mid_high: theme.spectrum_mid_high,
            high: theme.spectrum_high,
            treble: theme.spectrum_treble,
        }
    }

    /// Get color for frequency band index
    pub fn get_color(&self, band_index: usize) -> Color {
        match band_index {
            0 => self.sub_bass,
            1 => self.bass,
            2 => self.mid_low,
            3 => self.mid,
            4 => self.mid_high,
            5 => self.high,
            _ => self.treble,
        }
    }
}

impl Default for BandColors {
    fn default() -> Self {
        Self {
            sub_bass: Color::Red,
            bass: Color::LightRed,
            mid_low: Color::Yellow,
            mid: Color::Green,
            mid_high: Color::Cyan,
            high: Color::Blue,
            treble: Color::Magenta,
        }
    }
}

/// Visualizer state for animations
#[derive(Debug, Clone)]
pub struct VisualizerState {
    /// Current visualization mode
    pub mode: VisualizationMode,

    /// Last spectrum result
    pub spectrum: Option<SpectrumResult>,

    /// Smoothed bar heights for animation
    pub bar_heights: Vec<f32>,

    /// Target bar heights
    pub target_heights: Vec<f32>,

    /// Beat pulse intensity (0.0 to 1.0)
    pub beat_pulse: f32,

    /// Animation tick
    pub tick: u32,

    /// Number of bars to display
    pub num_bars: usize,
}

impl Default for VisualizerState {
    fn default() -> Self {
        Self {
            mode: VisualizationMode::Bars,
            spectrum: None,
            bar_heights: Vec::new(),
            target_heights: Vec::new(),
            beat_pulse: 0.0,
            tick: 0,
            num_bars: 32,
        }
    }
}

impl VisualizerState {
    /// Create new visualizer state
    pub fn new(num_bars: usize) -> Self {
        Self {
            num_bars,
            ..Default::default()
        }
    }

    /// Update with new spectrum data
    pub fn update(&mut self, spectrum: SpectrumResult) {
        self.spectrum = Some(spectrum.clone());
        self.tick = self.tick.wrapping_add(1);

        // Update beat pulse
        if spectrum.beat.beat_detected {
            self.beat_pulse = spectrum.beat.intensity;
        } else {
            // Decay beat pulse
            self.beat_pulse *= 0.9;
        }

        // Update bar heights
        self.update_bar_heights(&spectrum);
    }

    /// Update bar heights from spectrum
    fn update_bar_heights(&mut self, spectrum: &SpectrumResult) {
        let num_bars = self.num_bars;
        let magnitudes = &spectrum.magnitudes;

        // Resize if needed
        if self.bar_heights.len() != num_bars {
            self.bar_heights = vec![0.0; num_bars];
            self.target_heights = vec![0.0; num_bars];
        }

        // Calculate target heights by averaging magnitudes into bars
        let samples_per_bar = magnitudes.len() / num_bars;

        for i in 0..num_bars {
            let start = i * samples_per_bar;
            let end = ((i + 1) * samples_per_bar).min(magnitudes.len());

            if start < magnitudes.len() {
                let sum: f32 = magnitudes[start..end].iter().sum();
                self.target_heights[i] = sum / (end - start) as f32;
            }
        }

        // Smooth animation towards target
        let smoothing = 0.3;
        for i in 0..num_bars {
            self.bar_heights[i] =
                smoothing * self.target_heights[i] + (1.0 - smoothing) * self.bar_heights[i];
        }
    }

    /// Set visualization mode
    pub fn set_mode(&mut self, mode: VisualizationMode) {
        self.mode = mode;
    }

    /// Cycle to next visualization mode
    pub fn next_mode(&mut self) {
        self.mode = self.mode.next();
    }
}

/// Render visualizer to frame
pub fn render_visualizer(f: &mut Frame, area: Rect, state: &VisualizerState, theme: &Theme) {
    if state.mode == VisualizationMode::Off {
        return;
    }

    // Ensure minimum size
    if area.width < 10 || area.height < 5 {
        return;
    }

    let colors = BandColors::from_theme(theme);

    match state.mode {
        VisualizationMode::Bars => render_bars(f, area, state, &colors, theme),
        VisualizationMode::Waveform => render_waveform(f, area, state, &colors, theme),
        VisualizationMode::Circular => render_circular(f, area, state, &colors, theme),
        VisualizationMode::Off => {}
    }
}

/// Render bar visualization
fn render_bars(
    f: &mut Frame,
    area: Rect,
    state: &VisualizerState,
    colors: &BandColors,
    theme: &Theme,
) {
    let num_bars = state.num_bars.min(area.width as usize);
    if num_bars == 0 {
        return;
    }

    let bar_width = area.width as usize / num_bars;
    let max_height = area.height.saturating_sub(2) as usize;

    // Create block with title
    let title = format!(
        " 📊 SPECTRUM {} ",
        state
            .spectrum
            .as_ref()
            .and_then(|s| s.beat.estimated_bpm.map(|bpm| format!("~{:.0} BPM", bpm)))
            .unwrap_or_default()
    );

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border()));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.width < 2 || inner.height < 2 {
        return;
    }

    // Render each bar
    for (i, &height) in state.bar_heights.iter().take(num_bars).enumerate() {
        let x = inner.x + (i * bar_width) as u16;
        let bar_height = (height * inner.height as f32 * 0.9) as usize;
        let bar_height = bar_height.min(inner.height as usize);

        // Get color based on frequency band
        let band_idx = (i * 7) / num_bars;
        let color = colors.get_color(band_idx);

        // Add beat pulse effect
        let style = if state.beat_pulse > 0.3 && height > 0.5 {
            Style::default().fg(Color::White).bg(color)
        } else {
            Style::default().fg(color)
        };

        // Draw bar using block characters
        for y in 0..bar_height {
            let y_pos = inner.y + inner.height - 1 - y as u16;
            if y_pos >= inner.y && x < inner.x + inner.width {
                let char = if y == bar_height - 1 {
                    "▀" // Top of bar
                } else {
                    "█" // Full block
                };

                let span = Span::styled(char, style);
                f.render_widget(
                    Paragraph::new(span),
                    Rect::new(x, y_pos, bar_width as u16, 1),
                );
            }
        }
    }

    // Add beat pulse glow effect
    if state.beat_pulse > 0.5 {
        let pulse_char = "░";
        let pulse_style = Style::default().fg(theme.accent()).bg(theme.background());

        for y in 0..inner.height {
            let span = Span::styled(pulse_char, pulse_style);
            f.render_widget(Paragraph::new(span), Rect::new(inner.x, inner.y + y, 1, 1));
            f.render_widget(
                Paragraph::new(span),
                Rect::new(inner.x + inner.width - 1, inner.y + y, 1, 1),
            );
        }
    }
}

/// Render waveform visualization
fn render_waveform(
    f: &mut Frame,
    area: Rect,
    state: &VisualizerState,
    colors: &BandColors,
    theme: &Theme,
) {
    let block = Block::default()
        .title(" 〰️ WAVEFORM ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border()));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.width < 4 || inner.height < 3 {
        return;
    }

    // Use spectrum data to create pseudo-waveform display
    // (In a real implementation, we'd use raw time-domain samples)
    let spectrum = match &state.spectrum {
        Some(s) => s,
        None => return,
    };

    let mid_y = inner.y + inner.height / 2;
    let center_x = inner.x;
    let width = inner.width as usize;

    // Draw center line
    let center_style = Style::default().fg(theme.text_dim());
    for x in 0..width {
        let span = Span::styled("─", center_style);
        f.render_widget(
            Paragraph::new(span),
            Rect::new(center_x + x as u16, mid_y, 1, 1),
        );
    }

    // Draw waveform based on magnitudes
    let samples = 64.min(width);
    let samples_per_pixel = spectrum.magnitudes.len() / samples;

    for i in 0..samples {
        let start = i * samples_per_pixel;
        let end = ((i + 1) * samples_per_pixel).min(spectrum.magnitudes.len());

        if start >= spectrum.magnitudes.len() {
            break;
        }

        let avg: f32 = spectrum.magnitudes[start..end].iter().sum::<f32>() / (end - start) as f32;

        // Map to vertical position
        let amplitude = avg * inner.height as f32 / 2.0;
        let top_offset = amplitude as u16;

        // Draw waveform above and below center
        let color = colors.get_color((i * 7) / samples);
        let style = Style::default().fg(color);

        // Upper part
        if top_offset > 0 && mid_y >= top_offset {
            let span = Span::styled("█", style);
            f.render_widget(
                Paragraph::new(span),
                Rect::new(center_x + i as u16, mid_y - top_offset, 1, top_offset),
            );
        }

        // Lower part
        if top_offset > 0 {
            let span = Span::styled("█", style);
            f.render_widget(
                Paragraph::new(span),
                Rect::new(center_x + i as u16, mid_y + 1, 1, top_offset),
            );
        }
    }
}

/// Render circular visualization
fn render_circular(
    f: &mut Frame,
    area: Rect,
    state: &VisualizerState,
    colors: &BandColors,
    theme: &Theme,
) {
    let block = Block::default()
        .title(" ⭕ CIRCULAR ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border()));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.width < 8 || inner.height < 6 {
        return;
    }

    let spectrum = match &state.spectrum {
        Some(s) => s,
        None => return,
    };

    // Calculate center and radius
    let center_x = inner.x + inner.width / 2;
    let center_y = inner.y + inner.height / 2;
    let max_radius = (inner.width.min(inner.height * 2) / 2) as f32;
    let min_radius = max_radius * 0.3;

    // Use bands for circular visualization
    let bands = &spectrum.bands;
    let num_segments = bands.len();

    // Draw circular bars for each frequency band
    for (i, band) in bands.iter().enumerate() {
        let angle_start = (i as f32 / num_segments as f32) * 360.0;
        let angle_end = ((i + 1) as f32 / num_segments as f32) * 360.0;
        let angle_mid = (angle_start + angle_end) / 2.0;

        // Calculate bar length based on magnitude
        let magnitude = band.magnitude;
        let bar_length = min_radius + magnitude * (max_radius - min_radius);

        // Convert polar to cartesian
        let angle_rad = angle_mid.to_radians();

        // Inner point
        let inner_x = center_x as f32 + min_radius * angle_rad.cos();
        let inner_y = center_y as f32 + min_radius * angle_rad.sin() * 0.5; // Adjust for terminal aspect ratio

        // Outer point
        let outer_x = center_x as f32 + bar_length * angle_rad.cos();
        let outer_y = center_y as f32 + bar_length * angle_rad.sin() * 0.5;

        // Get color for this band
        let color = colors.get_color(i);
        let style = Style::default().fg(color);

        // Draw line from inner to outer (simplified - just draw endpoints)
        let outer_char = if magnitude > 0.7 {
            "●"
        } else if magnitude > 0.4 {
            "◉"
        } else {
            "○"
        };

        // Draw outer point
        let outer_px = outer_x as u16;
        let outer_py = outer_y as u16;

        if outer_px >= inner.x
            && outer_px < inner.x + inner.width
            && outer_py >= inner.y
            && outer_py < inner.y + inner.height
        {
            let span = Span::styled(outer_char, style);
            f.render_widget(Paragraph::new(span), Rect::new(outer_px, outer_py, 1, 1));
        }
    }

    // Draw center circle
    let center_style = Style::default().fg(theme.accent());
    let center_char = if state.beat_pulse > 0.5 { "◉" } else { "●" };
    let span = Span::styled(center_char, center_style);
    f.render_widget(Paragraph::new(span), Rect::new(center_x, center_y, 1, 1));

    // Add BPM display if available
    if let Some(bpm) = spectrum.beat.estimated_bpm {
        let bpm_text = format!("{:.0}", bpm);
        let span = Span::styled(bpm_text, Style::default().fg(theme.text_dim()));
        f.render_widget(
            Paragraph::new(span),
            Rect::new(center_x + 2, center_y, 4, 1),
        );
    }
}

/// Render mini spectrum (for status bar or small areas)
pub fn render_mini_spectrum(f: &mut Frame, area: Rect, magnitudes: &[f32], theme: &Theme) {
    if area.width < 4 || magnitudes.is_empty() {
        return;
    }

    let num_bars = (area.width as usize).min(magnitudes.len()).min(16);
    let samples_per_bar = magnitudes.len() / num_bars;

    for i in 0..num_bars {
        let start = i * samples_per_bar;
        let end = ((i + 1) * samples_per_bar).min(magnitudes.len());

        let avg: f32 = magnitudes[start..end].iter().sum::<f32>() / (end - start).max(1) as f32;

        // Choose character based on magnitude
        let char = if avg > 0.8 {
            "█"
        } else if avg > 0.6 {
            "▓"
        } else if avg > 0.4 {
            "▒"
        } else if avg > 0.2 {
            "░"
        } else {
            " "
        };

        let style = Style::default().fg(theme.accent());
        let span = Span::styled(char, style);
        f.render_widget(
            Paragraph::new(span),
            Rect::new(area.x + i as u16, area.y, 1, 1),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_visualization_mode_cycle() {
        let mode = VisualizationMode::Bars;
        assert_eq!(mode.next(), VisualizationMode::Waveform);
        assert_eq!(
            VisualizationMode::Waveform.next(),
            VisualizationMode::Circular
        );
        assert_eq!(VisualizationMode::Circular.next(), VisualizationMode::Off);
        assert_eq!(VisualizationMode::Off.next(), VisualizationMode::Bars);
    }

    #[test]
    fn test_visualizer_state_default() {
        let state = VisualizerState::default();
        assert_eq!(state.mode, VisualizationMode::Bars);
        assert!(state.spectrum.is_none());
        assert_eq!(state.beat_pulse, 0.0);
    }

    #[test]
    fn test_band_colors_default() {
        let colors = BandColors::default();
        assert_eq!(colors.sub_bass, Color::Red);
        assert_eq!(colors.bass, Color::LightRed);
        assert_eq!(colors.mid, Color::Green);
    }
}
