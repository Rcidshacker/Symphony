//! Visualizer UI Component
//!
//! Renders audio spectrum analysis in the terminal using Ratatui.
//! Supports multiple visualization modes with smooth animations.

use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::Span,
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::audio::analyzer::SpectrumResult;
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
    /// Minimal: one small level meter per frequency band
    Minimal,
    /// Visualizer hidden
    Off,
}

impl VisualizationMode {
    /// Get all available modes
    pub fn all() -> &'static [VisualizationMode] {
        &[
            VisualizationMode::Bars,
            VisualizationMode::Waveform,
            VisualizationMode::Circular,
            VisualizationMode::Minimal,
            VisualizationMode::Off,
        ]
    }

    /// Get display name
    pub fn name(&self) -> &'static str {
        match self {
            VisualizationMode::Bars => "Bars",
            VisualizationMode::Waveform => "Waveform",
            VisualizationMode::Circular => "Circular",
            VisualizationMode::Minimal => "Minimal",
            VisualizationMode::Off => "Off",
        }
    }

    /// Cycle to next mode
    pub fn next(&self) -> Self {
        match self {
            VisualizationMode::Bars => VisualizationMode::Waveform,
            VisualizationMode::Waveform => VisualizationMode::Circular,
            VisualizationMode::Circular => VisualizationMode::Minimal,
            VisualizationMode::Minimal => VisualizationMode::Off,
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
            sub_bass: theme.spectrum.sub_bass,
            bass: theme.spectrum.bass,
            mid_low: theme.spectrum.mid_low,
            mid: theme.spectrum.mid,
            mid_high: theme.spectrum.mid_high,
            high: theme.spectrum.high,
            treble: theme.spectrum.treble,
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
    ///
    /// Bars are log-spaced between 40 Hz and 16 kHz (the way pitch is heard); each bar shows
    /// the loudest bin in its range. Bars rise quickly and fall slowly.
    fn update_bar_heights(&mut self, spectrum: &SpectrumResult) {
        let num_bars = self.num_bars.max(1);
        let magnitudes = &spectrum.magnitudes;

        if self.bar_heights.len() != num_bars {
            self.bar_heights = vec![0.0; num_bars];
            self.target_heights = vec![0.0; num_bars];
        }
        if magnitudes.is_empty() || spectrum.bin_hz <= 0.0 {
            return;
        }

        let nyquist = spectrum.bin_hz * magnitudes.len() as f32;
        let f_lo = 40.0_f32;
        let f_hi = 16_000.0_f32.min(nyquist * 0.95).max(f_lo * 2.0);
        let edge = |i: usize| f_lo * (f_hi / f_lo).powf(i as f32 / num_bars as f32);

        for i in 0..num_bars {
            let start = ((edge(i) / spectrum.bin_hz) as usize).min(magnitudes.len() - 1);
            let end = ((edge(i + 1) / spectrum.bin_hz).ceil() as usize)
                .clamp(start + 1, magnitudes.len());
            self.target_heights[i] = magnitudes[start..end]
                .iter()
                .cloned()
                .fold(0.0_f32, f32::max);
        }

        for i in 0..num_bars {
            let target = self.target_heights[i];
            let current = self.bar_heights[i];
            let rate = if target > current { 0.6 } else { 0.2 };
            self.bar_heights[i] = current + (target - current) * rate;
        }
    }

    /// Fade out when no audio is playing (paused or stopped)
    pub fn decay(&mut self) {
        self.beat_pulse *= 0.9;
        for h in &mut self.bar_heights {
            *h *= 0.85;
        }
        if self.bar_heights.iter().all(|h| *h < 0.01) {
            self.bar_heights.iter_mut().for_each(|h| *h = 0.0);
            self.spectrum = None;
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
        VisualizationMode::Minimal => render_minimal(f, area, state, &colors, theme),
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
    let _max_height = area.height.saturating_sub(2) as usize;

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
            f.render_widget(
                Paragraph::new(span.clone()),
                Rect::new(inner.x, inner.y + y, 1, 1),
            );
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

    // Real time-domain samples: one column per point, mirrored around the center line
    let wave = &spectrum.waveform;
    if wave.is_empty() {
        return;
    }
    let half_height = inner.height as f32 / 2.0;
    for i in 0..width {
        let sample = wave[i * wave.len() / width];
        let half = ((sample.abs() * 1.5).min(1.0) * half_height) as u16;
        if half == 0 {
            continue;
        }
        let style = Style::default().fg(colors.get_color((i * 7) / width));
        let x = center_x + i as u16;
        let buf = f.buffer_mut();
        for dy in 1..=half {
            if mid_y >= inner.y + dy {
                buf.set_string(x, mid_y - dy, "█", style);
            }
            if mid_y + dy < inner.y + inner.height {
                buf.set_string(x, mid_y + dy, "█", style);
            }
        }
    }
}

/// Render minimal visualization: one small level meter per frequency band
fn render_minimal(
    f: &mut Frame,
    area: Rect,
    state: &VisualizerState,
    colors: &BandColors,
    theme: &Theme,
) {
    let beat = if state.beat_pulse > 0.5 { "●" } else { "○" };
    let block = Block::default()
        .title(format!(" ▪ LEVELS {beat} "))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border()));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let Some(spectrum) = &state.spectrum else {
        return;
    };
    const LABEL: usize = 9;
    if inner.width as usize <= LABEL + 2 || inner.height == 0 {
        return;
    }

    // Partial blocks give eighths of a cell
    const EIGHTHS: [&str; 8] = ["▏", "▎", "▍", "▌", "▋", "▊", "▉", "█"];
    let meter_width = inner.width as usize - LABEL - 1;

    for (row, band) in spectrum
        .bands
        .iter()
        .take(inner.height as usize)
        .enumerate()
    {
        let y = inner.y + row as u16;
        let filled_eighths = (band.magnitude.clamp(0.0, 1.0) * meter_width as f32 * 8.0) as usize;
        let mut meter = "█".repeat(filled_eighths / 8);
        if !filled_eighths.is_multiple_of(8) {
            meter.push_str(EIGHTHS[filled_eighths % 8 - 1]);
        }

        let buf = f.buffer_mut();
        buf.set_string(
            inner.x,
            y,
            format!("{:<width$}", band.name, width = LABEL),
            Style::default().fg(theme.text_dim()),
        );
        buf.set_string(
            inner.x + LABEL as u16,
            y,
            meter,
            Style::default().fg(colors.get_color(row)),
        );
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

    // Terminal cells are about twice as tall as wide: squash the y axis
    let center_x = inner.x as f32 + inner.width as f32 / 2.0;
    let center_y = inner.y as f32 + inner.height as f32 / 2.0;
    let max_radius = (inner.width as f32 / 2.0).min(inner.height as f32) - 0.5;
    let min_radius = max_radius * 0.3;

    // One spoke per bar, starting at 12 o'clock, drawn as a trail of dots from the hub outwards
    let num_spokes = state.bar_heights.len().max(1);
    for (i, height) in state.bar_heights.iter().enumerate() {
        let angle =
            (i as f32 / num_spokes as f32) * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
        let length = height.clamp(0.0, 1.0) * (max_radius - min_radius);
        let style = Style::default().fg(colors.get_color((i * 7) / num_spokes));

        let steps = (length * 2.0).ceil().max(1.0) as usize;
        for s in 0..=steps {
            let r = min_radius + length * s as f32 / steps as f32;
            let x = center_x + r * angle.cos();
            let y = center_y + r * angle.sin() * 0.5;
            if x >= inner.x as f32
                && y >= inner.y as f32
                && x < (inner.x + inner.width) as f32
                && y < (inner.y + inner.height) as f32
            {
                let tip = s == steps && length > 0.5;
                f.buffer_mut()
                    .set_string(x as u16, y as u16, if tip { "●" } else { "•" }, style);
            }
        }
    }

    // Hub pulses with the beat
    let hub = if state.beat_pulse > 0.5 { "◉" } else { "●" };
    f.buffer_mut().set_string(
        center_x as u16,
        center_y as u16,
        hub,
        Style::default().fg(theme.accent()),
    );

    // Tempo in the corner when known
    if let Some(bpm) = spectrum.beat.estimated_bpm {
        f.buffer_mut().set_string(
            inner.x + 1,
            inner.y + inner.height - 1,
            format!("{bpm:.0} BPM"),
            Style::default().fg(theme.text_dim()),
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
        assert_eq!(
            VisualizationMode::Circular.next(),
            VisualizationMode::Minimal
        );
        assert_eq!(VisualizationMode::Minimal.next(), VisualizationMode::Off);
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
