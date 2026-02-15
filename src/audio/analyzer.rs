//! Real-time FFT Spectrum Analyzer
//!
//! High-performance audio spectrum analysis using RustFFT.
//! Provides frequency-domain data for visualization.

use std::collections::VecDeque;
use std::f32::consts::PI;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use rustfft::{FftPlanner, FftDirection};
use rustfft::num_complex::Complex;
use rustfft::num_traits::Zero;

/// FFT window size options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FftSize {
    /// 2048 samples - faster, less resolution
    Size2048 = 2048,
    /// 4096 samples - balanced (default)
    Size4096 = 4096,
    /// 8192 samples - slower, more resolution
    Size8192 = 8192,
}

impl Default for FftSize {
    fn default() -> Self {
        Self::Size4096
    }
}

impl FftSize {
    /// Get the numeric size
    pub fn size(&self) -> usize {
        *self as usize
    }

    /// Get frequency resolution at given sample rate
    pub fn frequency_resolution(&self, sample_rate: u32) -> f32 {
        sample_rate as f32 / self.size() as f32
    }

    /// Get frequency bin index for a given frequency
    pub fn bin_for_frequency(&self, frequency: f32, sample_rate: u32) -> usize {
        let bin = (frequency / self.frequency_resolution(sample_rate)) as usize;
        bin.min(self.size() / 2 - 1)
    }
}

/// Window function type for FFT
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WindowFunction {
    /// Hanning window (default, good for most cases)
    Hanning,
    /// Hamming window (better for speech)
    Hamming,
    /// Blackman window (better side-lobe suppression)
    Blackman,
    /// Rectangular (no window)
    Rectangular,
}

impl Default for WindowFunction {
    fn default() -> Self {
        Self::Hanning
    }
}

/// Frequency band for spectrum analysis
#[derive(Debug, Clone)]
pub struct FrequencyBand {
    /// Band name
    pub name: String,
    /// Low frequency (Hz)
    pub low: f32,
    /// High frequency (Hz)
    pub high: f32,
    /// Average magnitude in this band
    pub magnitude: f32,
}

/// Predefined frequency bands
pub fn get_frequency_bands() -> Vec<FrequencyBand> {
    vec![
        FrequencyBand { name: "Sub Bass".into(), low: 20.0, high: 60.0, magnitude: 0.0 },
        FrequencyBand { name: "Bass".into(), low: 60.0, high: 200.0, magnitude: 0.0 },
        FrequencyBand { name: "Mid-Low".into(), low: 200.0, high: 500.0, magnitude: 0.0 },
        FrequencyBand { name: "Mid".into(), low: 500.0, high: 2000.0, magnitude: 0.0 },
        FrequencyBand { name: "Mid-High".into(), low: 2000.0, high: 4000.0, magnitude: 0.0 },
        FrequencyBand { name: "High".into(), low: 4000.0, high: 8000.0, magnitude: 0.0 },
        FrequencyBand { name: "Treble".into(), low: 8000.0, high: 20000.0, magnitude: 0.0 },
    ]
}

/// Beat detection result
#[derive(Debug, Clone)]
pub struct BeatResult {
    /// Whether a beat was detected
    pub beat_detected: bool,
    /// Beat intensity (0.0 to 1.0)
    pub intensity: f32,
    /// Estimated BPM (if available)
    pub estimated_bpm: Option<f32>,
}

/// Spectrum analysis result
#[derive(Debug, Clone)]
pub struct SpectrumResult {
    /// Magnitude spectrum (normalized 0.0 to 1.0)
    pub magnitudes: Vec<f32>,
    /// Frequency bands with average magnitudes
    pub bands: Vec<FrequencyBand>,
    /// Beat detection result
    pub beat: BeatResult,
    /// Peak frequency (Hz)
    pub peak_frequency: f32,
    /// RMS level
    pub rms: f32,
}

/// Real-time FFT Spectrum Analyzer
pub struct SpectrumAnalyzer {
    /// FFT size
    fft_size: FftSize,

    /// Sample rate
    sample_rate: u32,

    /// Window function coefficients
    window: Vec<f32>,

    /// Window function type
    window_type: WindowFunction,

    /// FFT input buffer
    input_buffer: Vec<Complex<f32>>,

    /// FFT output buffer
    output_buffer: Vec<Complex<f32>>,

    /// FFT planner (cached for performance)
    fft_planner: FftPlanner<f32>,

    /// Sample ring buffer for continuous analysis
    sample_buffer: VecDeque<f32>,

    /// Previous spectrum for smoothing
    previous_spectrum: Vec<f32>,

    /// Smoothing factor (0.0 to 1.0)
    smoothing: f32,

    /// Beat detection state
    beat_detector: BeatDetector,

    /// Frequency bands for analysis
    frequency_bands: Vec<FrequencyBand>,

    /// Whether analysis is running
    running: Arc<AtomicBool>,

    /// Frame counter for performance monitoring
    frame_count: Arc<AtomicUsize>,
}

impl SpectrumAnalyzer {
    /// Create a new spectrum analyzer
    pub fn new(fft_size: FftSize, sample_rate: u32) -> Self {
        let size = fft_size.size();

        // Create window function
        let window_type = WindowFunction::default();
        let window = Self::create_window(size, window_type);

        // Initialize buffers
        let input_buffer = vec![Complex::zero(); size];
        let output_buffer = vec![Complex::zero(); size];

        // Create FFT planner
        let fft_planner = FftPlanner::new();

        // Initialize sample buffer
        let sample_buffer = VecDeque::with_capacity(size * 2);

        // Initialize previous spectrum for smoothing
        let previous_spectrum = vec![0.0; size / 2];

        Self {
            fft_size,
            sample_rate,
            window,
            window_type,
            input_buffer,
            output_buffer,
            fft_planner,
            sample_buffer,
            previous_spectrum,
            smoothing: 0.7,
            beat_detector: BeatDetector::new(),
            frequency_bands: get_frequency_bands(),
            running: Arc::new(AtomicBool::new(false)),
            frame_count: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Create window function coefficients
    fn create_window(size: usize, window_type: WindowFunction) -> Vec<f32> {
        match window_type {
            WindowFunction::Hanning => {
                // Hanning window: 0.5 * (1 - cos(2*pi*n / (N-1)))
                (0..size)
                    .map(|n| 0.5 * (1.0 - ((2.0 * PI * n as f32) / (size - 1) as f32).cos()))
                    .collect()
            }
            WindowFunction::Hamming => {
                // Hamming window: 0.54 - 0.46 * cos(2*pi*n / (N-1))
                (0..size)
                    .map(|n| 0.54 - 0.46 * ((2.0 * PI * n as f32) / (size - 1) as f32).cos())
                    .collect()
            }
            WindowFunction::Blackman => {
                // Blackman window
                (0..size)
                    .map(|n| {
                        let t = (2.0 * PI * n as f32) / (size - 1) as f32;
                        0.42 - 0.5 * t.cos() + 0.08 * (2.0 * t).cos()
                    })
                    .collect()
            }
            WindowFunction::Rectangular => {
                vec![1.0; size]
            }
        }
    }

    /// Set smoothing factor
    pub fn set_smoothing(&mut self, smoothing: f32) {
        self.smoothing = smoothing.clamp(0.0, 0.99);
    }

    /// Set window function
    pub fn set_window(&mut self, window_type: WindowFunction) {
        self.window_type = window_type;
        self.window = Self::create_window(self.fft_size.size(), window_type);
    }

    /// Add samples to the analysis buffer
    pub fn add_samples(&mut self, samples: &[f32]) {
        for sample in samples {
            self.sample_buffer.push_back(*sample);
        }

        // Keep buffer at reasonable size
        while self.sample_buffer.len() > self.fft_size.size() * 4 {
            self.sample_buffer.pop_front();
        }
    }

    /// Check if enough samples are available for analysis
    pub fn has_enough_samples(&self) -> bool {
        self.sample_buffer.len() >= self.fft_size.size()
    }

    /// Perform FFT analysis on available samples
    pub fn analyze(&mut self) -> Option<SpectrumResult> {
        if !self.has_enough_samples() {
            return None;
        }

        let size = self.fft_size.size();

        // Copy samples to input buffer with windowing
        let samples: Vec<f32> = self.sample_buffer.iter()
            .take(size)
            .copied()
            .collect();

        for i in 0..size {
            let sample = samples.get(i).copied().unwrap_or(0.0);
            let window_val = self.window[i];
            self.input_buffer[i] = Complex::new(sample * window_val, 0.0);
        }

        // Perform FFT
        let fft = self.fft_planner.plan_fft_forward(size);
        self.output_buffer.copy_from_slice(&self.input_buffer);
        fft.process(&mut self.output_buffer);

        // Calculate magnitude spectrum (only positive frequencies)
        let half_size = size / 2;
        let mut magnitudes = Vec::with_capacity(half_size);

        for i in 0..half_size {
            let complex = self.output_buffer[i];
            let magnitude = (complex.re.powi(2) + complex.im.powi(2)).sqrt();
            // Convert to dB and normalize
            let db = 20.0 * magnitude.max(1e-10).log10();
            let normalized = (db + 100.0) / 100.0; // Normalize from -100dB to 0dB
            magnitudes.push(normalized.clamp(0.0, 1.0));
        }

        // Apply smoothing
        for i in 0..half_size {
            magnitudes[i] = self.smoothing * self.previous_spectrum[i]
                + (1.0 - self.smoothing) * magnitudes[i];
        }
        self.previous_spectrum.copy_from_slice(&magnitudes);

        // Calculate frequency bands
        let bands = self.calculate_frequency_bands(&magnitudes);

        // Calculate RMS
        let rms = Self::calculate_rms(&samples);

        // Detect beats
        let beat = self.beat_detector.detect(&bands, rms);

        // Find peak frequency
        let peak_freq = self.find_peak_frequency(&magnitudes);

        // Increment frame count
        self.frame_count.fetch_add(1, Ordering::Relaxed);

        Some(SpectrumResult {
            magnitudes,
            bands,
            beat,
            peak_frequency: peak_freq,
            rms,
        })
    }

    /// Calculate average magnitude for frequency bands
    fn calculate_frequency_bands(&self, magnitudes: &[f32]) -> Vec<FrequencyBand> {
        let mut bands = get_frequency_bands();
        let freq_resolution = self.fft_size.frequency_resolution(self.sample_rate);

        for band in &mut bands {
            let low_bin = (band.low / freq_resolution) as usize;
            let high_bin = (band.high / freq_resolution) as usize;

            let low_bin = low_bin.min(magnitudes.len() - 1);
            let high_bin = high_bin.min(magnitudes.len() - 1);

            if high_bin > low_bin {
                let sum: f32 = magnitudes[low_bin..high_bin].iter().sum();
                band.magnitude = sum / (high_bin - low_bin) as f32;
            } else {
                band.magnitude = magnitudes.get(low_bin).copied().unwrap_or(0.0);
            }
        }

        bands
    }

    /// Find peak frequency in spectrum
    fn find_peak_frequency(&self, magnitudes: &[f32]) -> f32 {
        let max_idx = magnitudes.iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, _)| i)
            .unwrap_or(0);

        let freq_resolution = self.fft_size.frequency_resolution(self.sample_rate);
        max_idx as f32 * freq_resolution
    }

    /// Calculate RMS of samples
    fn calculate_rms(samples: &[f32]) -> f32 {
        if samples.is_empty() {
            return 0.0;
        }
        let sum: f32 = samples.iter().map(|s| s * s).sum();
        (sum / samples.len() as f32).sqrt()
    }

    /// Get FFT size
    pub fn fft_size(&self) -> FftSize {
        self.fft_size
    }

    /// Get sample rate
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Get frame count
    pub fn frame_count(&self) -> usize {
        self.frame_count.load(Ordering::Relaxed)
    }

    /// Reset analyzer state
    pub fn reset(&mut self) {
        self.sample_buffer.clear();
        self.previous_spectrum.fill(0.0);
        self.beat_detector.reset();
        self.frame_count.store(0, Ordering::Relaxed);
    }
}

/// Beat detection algorithm
#[derive(Debug)]
struct BeatDetector {
    /// Energy history for beat detection
    energy_history: VecDeque<f32>,

    /// History size
    history_size: usize,

    /// Sensitivity (0.0 to 1.0)
    sensitivity: f32,

    /// Last beat time
    last_beat_time: std::time::Instant,

    /// Beat intervals for BPM calculation
    beat_intervals: VecDeque<std::time::Duration>,

    /// Estimated BPM
    estimated_bpm: f32,

    /// Threshold multiplier
    threshold_mult: f32,
}

impl BeatDetector {
    fn new() -> Self {
        Self {
            energy_history: VecDeque::with_capacity(44),
            history_size: 44, // ~1 second at 44.1kHz/1024 hop
            sensitivity: 0.5,
            last_beat_time: std::time::Instant::now(),
            beat_intervals: VecDeque::with_capacity(10),
            estimated_bpm: 0.0,
            threshold_mult: 1.5,
        }
    }

    /// Detect beat from frequency bands and RMS
    fn detect(&mut self, bands: &[FrequencyBand], rms: f32) -> BeatResult {
        // Calculate bass energy (primary beat indicator)
        let bass_energy = bands.iter()
            .take(3) // Sub bass, Bass, Mid-Low
            .map(|b| b.magnitude)
            .sum::<f32>() / 3.0;

        // Add to history
        self.energy_history.push_back(bass_energy);
        if self.energy_history.len() > self.history_size {
            self.energy_history.pop_front();
        }

        // Calculate average and variance
        let avg: f32 = self.energy_history.iter().sum::<f32>() / self.energy_history.len().max(1) as f32;
        let variance: f32 = self.energy_history.iter()
            .map(|e| (e - avg).powi(2))
            .sum::<f32>() / self.energy_history.len().max(1) as f32;

        // Adaptive threshold
        let threshold = avg + self.threshold_mult * variance.sqrt();

        // Beat detected if current energy exceeds threshold
        let beat_detected = bass_energy > threshold
            && bass_energy > 0.3
            && self.last_beat_time.elapsed().as_millis() > 200; // Min 200ms between beats

        let intensity = if beat_detected {
            // Update beat timing
            let now = std::time::Instant::now();
            let interval = now - self.last_beat_time;
            self.last_beat_time = now;

            // Update BPM estimate
            self.beat_intervals.push_back(interval);
            if self.beat_intervals.len() > 10 {
                self.beat_intervals.pop_front();
            }

            if self.beat_intervals.len() >= 3 {
                let avg_interval: f32 = self.beat_intervals.iter()
                    .map(|d| d.as_secs_f32())
                    .sum::<f32>() / self.beat_intervals.len() as f32;
                self.estimated_bpm = 60.0 / avg_interval;
            }

            (bass_energy / threshold).min(1.0)
        } else {
            0.0
        };

        BeatResult {
            beat_detected,
            intensity,
            estimated_bpm: if self.estimated_bpm > 0.0 && self.estimated_bpm < 300.0 {
                Some(self.estimated_bpm)
            } else {
                None
            },
        }
    }

    /// Set sensitivity (0.0 to 1.0)
    fn set_sensitivity(&mut self, sensitivity: f32) {
        self.sensitivity = sensitivity.clamp(0.0, 1.0);
        self.threshold_mult = 1.0 + (1.0 - self.sensitivity) * 2.0;
    }

    /// Reset beat detector
    fn reset(&mut self) {
        self.energy_history.clear();
        self.beat_intervals.clear();
        self.estimated_bpm = 0.0;
    }
}

/// Audio sample provider trait (for integration with audio engine)
pub trait AudioSampleProvider: Send + Sync {
    /// Get the latest audio samples
    fn get_samples(&self, buffer: &mut [f32]) -> usize;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Generate a sine wave for testing
    fn generate_sine_wave(frequency: f32, sample_rate: u32, duration_secs: f32) -> Vec<f32> {
        let num_samples = (sample_rate as f32 * duration_secs) as usize;
        (0..num_samples)
            .map(|i| {
                let t = i as f32 / sample_rate as f32;
                (2.0 * PI * frequency * t).sin()
            })
            .collect()
    }

    #[test]
    fn test_fft_size_frequency_resolution() {
        let fft_size = FftSize::Size4096;
        let resolution = fft_size.frequency_resolution(44100);

        // At 44100 Hz with 4096 samples, resolution should be ~10.77 Hz
        assert!((resolution - 10.7666016).abs() < 0.01);
    }

    #[test]
    fn test_window_functions() {
        let size = 1024;

        // Hanning window should be 0 at edges
        let hanning = SpectrumAnalyzer::create_window(size, WindowFunction::Hanning);
        assert!(hanning[0] < 0.01);
        assert!(hanning[size - 1] < 0.01);

        // Hanning should be 1 at center
        let center = hanning[size / 2];
        assert!((center - 1.0).abs() < 0.01);

        // Rectangular should be all 1s
        let rect = SpectrumAnalyzer::create_window(size, WindowFunction::Rectangular);
        assert!(rect.iter().all(|&x| x == 1.0));
    }

    #[test]
    fn test_sine_wave_peak_detection() {
        let mut analyzer = SpectrumAnalyzer::new(FftSize::Size4096, 44100);

        // Generate 440 Hz sine wave (A4 note)
        let samples = generate_sine_wave(440.0, 44100, 0.5);
        analyzer.add_samples(&samples);

        let result = analyzer.analyze();
        assert!(result.is_some());

        let result = result.unwrap();
        // Peak should be near 440 Hz (within some tolerance)
        assert!((result.peak_frequency - 440.0).abs() < 20.0);
    }

    #[test]
    fn test_rms_calculation() {
        // RMS of a sine wave with amplitude 1 is 1/sqrt(2) ≈ 0.707
        let samples = generate_sine_wave(440.0, 44100, 1.0);
        let rms = SpectrumAnalyzer::calculate_rms(&samples);

        assert!((rms - 0.707).abs() < 0.01);
    }

    #[test]
    fn test_frequency_bands() {
        let bands = get_frequency_bands();
        assert_eq!(bands.len(), 7);

        // Bass should cover 60-200 Hz
        let bass = &bands[1];
        assert_eq!(bass.name, "Bass");
        assert_eq!(bass.low, 60.0);
        assert_eq!(bass.high, 200.0);
    }

    #[test]
    fn test_spectrum_smoothing() {
        let mut analyzer = SpectrumAnalyzer::new(FftSize::Size2048, 44100);
        analyzer.set_smoothing(0.5);

        // Generate some audio
        let samples = generate_sine_wave(1000.0, 44100, 0.2);

        // First analysis
        analyzer.add_samples(&samples);
        let result1 = analyzer.analyze().unwrap();

        // Second analysis with different frequency
        analyzer.reset();
        let samples2 = generate_sine_wave(500.0, 44100, 0.2);
        analyzer.add_samples(&samples2);
        let result2 = analyzer.analyze().unwrap();

        // Results should be different (smoothing doesn't prevent change, just smooths it)
        assert!(result1.peak_frequency != result2.peak_frequency);
    }

    #[test]
    fn test_performance() {
        use std::time::Instant;

        let mut analyzer = SpectrumAnalyzer::new(FftSize::Size4096, 44100);

        // Generate 1 second of audio
        let samples = generate_sine_wave(440.0, 44100, 1.0);

        // Measure FFT processing time
        let start = Instant::now();
        let num_frames = 100;

        for _ in 0..num_frames {
            analyzer.add_samples(&samples[..4096]);
            let _ = analyzer.analyze();
        }

        let elapsed = start.elapsed();
        let avg_time = elapsed.as_micros() as f64 / num_frames as f64;

        println!("Average FFT time: {:.2} µs", avg_time);

        // Should be less than 5ms per FFT
        assert!(avg_time < 5000.0, "FFT took too long: {:.2} µs", avg_time);
    }
}
