//! FFT Performance Benchmarks
//!
//! Benchmark the FFT spectrum analyzer performance.

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};

/// Generate a sine wave
fn generate_sine_wave(frequency: f32, sample_rate: u32, num_samples: usize) -> Vec<f32> {
    use std::f32::consts::PI;
    (0..num_samples)
        .map(|i| {
            let t = i as f32 / sample_rate as f32;
            (2.0 * PI * frequency * t).sin()
        })
        .collect()
}

/// Benchmark FFT with different sizes
fn fft_benchmark(c: &mut Criterion) {
    use rustfft::{FftPlanner, num_complex::Complex};

    let mut group = c.benchmark_group("FFT");

    for &size in &[2048, 4096, 8192] {
        let samples: Vec<f32> = generate_sine_wave(440.0, 44100, size);
        let mut buffer: Vec<Complex<f32>> = samples.iter()
            .map(|&s| Complex::new(s, 0.0))
            .collect();

        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(size);

        group.bench_with_input(BenchmarkId::new("size", size), &size, |b, _| {
            b.iter(|| {
                fft.process(black_box(&mut buffer));
            });
        });
    }

    group.finish();
}

/// Benchmark complete spectrum analysis
fn spectrum_analysis_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("SpectrumAnalysis");

    for &size in &[2048, 4096, 8192] {
        let fft_size = match size {
            2048 => symphony::audio::FftSize::Size2048,
            4096 => symphony::audio::FftSize::Size4096,
            8192 => symphony::audio::FftSize::Size8192,
            _ => continue,
        };

        let mut analyzer = symphony::audio::SpectrumAnalyzer::new(fft_size, 44100);
        let samples = generate_sine_wave(440.0, 44100, size * 2);

        group.bench_with_input(BenchmarkId::new("complete", size), &size, |b, _| {
            b.iter(|| {
                analyzer.add_samples(black_box(&samples));
                let _ = analyzer.analyze();
            });
        });
    }

    group.finish();
}

/// Benchmark visualizer rendering
fn visualizer_rendering_benchmark(c: &mut Criterion) {
    use symphony::audio::{FftSize, SpectrumAnalyzer};
    use symphony::ui::visualizer::VisualizerState;

    let mut group = c.benchmark_group("VisualizerRendering");

    let mut analyzer = SpectrumAnalyzer::new(FftSize::Size4096, 44100);
    let samples = generate_sine_wave(440.0, 44100, 4096);

    analyzer.add_samples(&samples);
    let spectrum = analyzer.analyze().unwrap();

    let mut state = VisualizerState::new(32);

    group.bench_function("update_state", |b| {
        b.iter(|| {
            let mut s = state.clone();
            s.update(black_box(spectrum.clone()));
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    fft_benchmark,
    spectrum_analysis_benchmark,
    visualizer_rendering_benchmark,
);

criterion_main!(benches);
