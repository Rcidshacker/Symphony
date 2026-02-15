//! Visualizer Tests
//!
//! Tests for FFT spectrum analyzer and visualization components.

#[cfg(test)]
mod tests {
    use std::f32::consts::PI;

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

    /// Generate white noise for testing
    fn generate_white_noise(num_samples: usize) -> Vec<f32> {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        (0..num_samples).map(|_| rng.gen_range(-1.0..1.0)).collect()
    }

    #[test]
    fn test_fft_size_calculations() {
        use symphony::audio::FftSize;

        let fft_size = FftSize::Size4096;
        assert_eq!(fft_size.size(), 4096);

        let resolution = fft_size.frequency_resolution(44100);
        assert!((resolution - 10.7666).abs() < 0.1);

        let bin = fft_size.bin_for_frequency(440.0, 44100);
        assert!((40..42).contains(&bin)); // ~440 Hz at bin ~41
    }

    #[test]
    fn test_spectrum_analyzer_creation() {
        use symphony::audio::{FftSize, SpectrumAnalyzer};

        let analyzer = SpectrumAnalyzer::new(FftSize::Size2048, 44100);
        assert_eq!(analyzer.fft_size(), FftSize::Size2048);
        assert_eq!(analyzer.sample_rate(), 44100);
        assert!(!analyzer.has_enough_samples());
    }

    #[test]
    fn test_sine_wave_peak_detection() {
        use symphony::audio::{FftSize, SpectrumAnalyzer};

        let mut analyzer = SpectrumAnalyzer::new(FftSize::Size4096, 44100);

        // Generate 440 Hz sine wave
        let samples = generate_sine_wave(440.0, 44100, 0.2);
        analyzer.add_samples(&samples);

        let result = analyzer.analyze();
        assert!(result.is_some());

        let spectrum = result.unwrap();
        // Peak should be near 440 Hz
        assert!((spectrum.peak_frequency - 440.0).abs() < 20.0);
    }

    #[test]
    fn test_frequency_bands() {
        use symphony::audio::get_frequency_bands;

        let bands = get_frequency_bands();
        assert_eq!(bands.len(), 7);

        // Check band ranges
        assert_eq!(bands[0].name, "Sub Bass");
        assert_eq!(bands[0].low, 20.0);
        assert_eq!(bands[0].high, 60.0);

        assert_eq!(bands[3].name, "Mid");
        assert_eq!(bands[3].low, 500.0);
        assert_eq!(bands[3].high, 2000.0);
    }

    #[test]
    fn test_visualization_mode_cycle() {
        use symphony::ui::visualizer::VisualizationMode;

        assert_eq!(VisualizationMode::Bars.next(), VisualizationMode::Waveform);
        assert_eq!(
            VisualizationMode::Waveform.next(),
            VisualizationMode::Circular
        );
        assert_eq!(VisualizationMode::Circular.next(), VisualizationMode::Off);
        assert_eq!(VisualizationMode::Off.next(), VisualizationMode::Bars);
    }

    #[test]
    fn test_visualizer_state() {
        use symphony::ui::visualizer::VisualizerState;

        let state = VisualizerState::new(32);
        assert_eq!(state.mode, VisualizationMode::Bars);
        assert!(state.spectrum.is_none());

        let mut state = state;
        state.next_mode();
        assert_eq!(state.mode, VisualizationMode::Waveform);
    }

    #[test]
    fn test_rms_calculation() {
        // RMS of a sine wave with amplitude 1 is 1/sqrt(2) ≈ 0.707
        let samples = generate_sine_wave(440.0, 44100, 1.0);

        let sum: f32 = samples.iter().map(|s| s * s).sum();
        let rms = (sum / samples.len() as f32).sqrt();

        assert!((rms - 0.707).abs() < 0.01);
    }

    #[test]
    fn test_performance() {
        use std::time::Instant;
        use symphony::audio::{FftSize, SpectrumAnalyzer};

        let mut analyzer = SpectrumAnalyzer::new(FftSize::Size4096, 44100);
        let samples = generate_sine_wave(440.0, 44100, 0.5);

        let start = Instant::now();
        let iterations = 100;

        for _ in 0..iterations {
            analyzer.add_samples(&samples[..4096]);
            let _ = analyzer.analyze();
        }

        let elapsed = start.elapsed();
        let avg_us = elapsed.as_micros() as f64 / iterations as f64;

        println!("Average FFT processing time: {:.2} µs", avg_us);

        // Should be less than 5ms per FFT
        assert!(
            avg_us < 5000.0,
            "FFT processing took too long: {:.2} µs",
            avg_us
        );
    }
}
