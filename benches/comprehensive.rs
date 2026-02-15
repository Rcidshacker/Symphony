//! Comprehensive Performance Benchmarks
//!
//! Benchmark all critical paths in Symphony.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::hint::black_box as bb;

// ============================================================================
// Helper Functions
// ============================================================================

/// Generate a sine wave at a given frequency
fn generate_sine_wave(frequency: f32, sample_rate: u32, num_samples: usize) -> Vec<f32> {
    use std::f32::consts::PI;
    (0..num_samples)
        .map(|i| {
            let t = i as f32 / sample_rate as f32;
            (2.0 * PI * frequency * t).sin()
        })
        .collect()
}

/// Generate noise
fn generate_noise(num_samples: usize) -> Vec<f32> {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    (0..num_samples)
        .map(|_| rng.gen::<f32>() * 2.0 - 1.0)
        .collect()
}

// ============================================================================
// FFT Benchmarks
// ============================================================================

fn bench_fft(c: &mut Criterion) {
    use rustfft::{num_complex::Complex, FftPlanner};

    let mut group = c.benchmark_group("FFT");
    group.measurement_time(std::time::Duration::from_secs(5));

    for &size in &[1024, 2048, 4096, 8192] {
        let samples: Vec<f32> = generate_sine_wave(440.0, 44100, size);
        let mut buffer: Vec<Complex<f32>> = samples.iter().map(|&s| Complex::new(s, 0.0)).collect();

        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(size);

        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(BenchmarkId::new("forward", size), &size, |b, _| {
            b.iter(|| {
                fft.process(black_box(&mut buffer));
            });
        });
    }

    group.finish();
}

// ============================================================================
// Spectrum Analysis Benchmarks
// ============================================================================

fn bench_spectrum_analysis(c: &mut Criterion) {
    let mut group = c.benchmark_group("SpectrumAnalysis");
    group.measurement_time(std::time::Duration::from_secs(5));

    // Benchmark raw FFT with windowing
    for &size in &[2048, 4096, 8192] {
        let samples: Vec<f32> = generate_sine_wave(440.0, 44100, size);

        // Hanning window
        let window: Vec<f32> = (0..size)
            .map(|i| {
                0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / (size - 1) as f32).cos())
            })
            .collect();

        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(BenchmarkId::new("windowed", size), &size, |b, _| {
            b.iter(|| {
                // Apply window
                let windowed: Vec<f32> = samples
                    .iter()
                    .zip(window.iter())
                    .map(|(s, w)| s * w)
                    .collect();
                black_box(windowed);
            });
        });
    }

    group.finish();
}

// ============================================================================
// Cache Operations Benchmarks
// ============================================================================

fn bench_cache_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("Cache");

    // Simulate LRU cache operations
    use std::collections::VecDeque;
    const CACHE_SIZE: usize = 100;

    group.bench_function("lru_get", |b| {
        let mut cache: VecDeque<(String, Vec<u8>)> = VecDeque::with_capacity(CACHE_SIZE);

        // Pre-populate
        for i in 0..CACHE_SIZE {
            cache.push_back((format!("key_{}", i), vec![0u8; 1024]));
        }

        b.iter(|| {
            let key = format!("key_{}", rand::random::<usize>() % CACHE_SIZE);
            if let Some(pos) = cache.iter().position(|(k, _)| k == &key) {
                let entry = cache.remove(pos).unwrap();
                cache.push_front(entry);
            }
            black_box(&cache);
        });
    });

    group.bench_function("lru_insert", |b| {
        let mut cache: VecDeque<(String, Vec<u8>)> = VecDeque::with_capacity(CACHE_SIZE);

        b.iter(|| {
            let key = format!("key_{}", rand::random::<usize>());
            let value = vec![0u8; 1024];

            if cache.len() >= CACHE_SIZE {
                cache.pop_back();
            }
            cache.push_front((key, value));
            black_box(&cache);
        });
    });

    group.finish();
}

// ============================================================================
// JSON Serialization Benchmarks
// ============================================================================

fn bench_json_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("JSON");

    #[derive(serde::Serialize, serde::Deserialize)]
    struct TrackInfo {
        id: String,
        title: String,
        artist: String,
        album: Option<String>,
        duration: u64,
        position: u64,
    }

    let track = TrackInfo {
        id: "track-123".to_string(),
        title: "Paranoid Android".to_string(),
        artist: "Radiohead".to_string(),
        album: Some("OK Computer".to_string()),
        duration: 383,
        position: 201,
    };

    group.bench_function("serialize", |b| {
        b.iter(|| serde_json::to_string(black_box(&track)).unwrap());
    });

    let json = serde_json::to_string(&track).unwrap();

    group.bench_function("deserialize", |b| {
        b.iter(|| {
            let _: TrackInfo = serde_json::from_str(black_box(&json)).unwrap();
        });
    });

    group.finish();
}

// ============================================================================
// String Operations Benchmarks
// ============================================================================

fn bench_string_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("String");

    let strings: Vec<String> = (0..1000)
        .map(|i| format!("track_{:04}_artist_name_song_title", i))
        .collect();

    group.bench_function("search_prefix", |b| {
        b.iter(|| {
            let results: Vec<_> = strings
                .iter()
                .filter(|s| s.starts_with("track_00"))
                .collect();
            black_box(results);
        });
    });

    group.bench_function("search_contains", |b| {
        b.iter(|| {
            let results: Vec<_> = strings.iter().filter(|s| s.contains("artist")).collect();
            black_box(results);
        });
    });

    group.finish();
}

// ============================================================================
// Hash Map Operations Benchmarks
// ============================================================================

fn bench_hashmap_operations(c: &mut Criterion) {
    use std::collections::HashMap;

    let mut group = c.benchmark_group("HashMap");

    // Pre-populate a hashmap
    let mut map: HashMap<String, Vec<u8>> = HashMap::new();
    for i in 0..10000 {
        map.insert(format!("key_{}", i), vec![0u8; 64]);
    }

    group.bench_function("lookup_hit", |b| {
        b.iter(|| {
            let key = format!("key_{}", rand::random::<usize>() % 10000);
            black_box(map.get(&key));
        });
    });

    group.bench_function("lookup_miss", |b| {
        b.iter(|| {
            black_box(map.get("nonexistent_key"));
        });
    });

    group.bench_function("insert", |b| {
        let mut map = HashMap::new();
        b.iter(|| {
            let key = format!("key_{}", rand::random::<usize>());
            map.insert(key, vec![0u8; 64]);
            black_box(&map);
        });
    });

    group.finish();
}

// ============================================================================
// Memory Allocation Benchmarks
// ============================================================================

fn bench_allocations(c: &mut Criterion) {
    let mut group = c.benchmark_group("Allocation");

    group.bench_function("vec_1k", |b| {
        b.iter(|| {
            let v: Vec<u8> = vec![0; 1024];
            black_box(v);
        });
    });

    group.bench_function("vec_16k", |b| {
        b.iter(|| {
            let v: Vec<u8> = vec![0; 16384];
            black_box(v);
        });
    });

    group.bench_function("vec_1m", |b| {
        b.iter(|| {
            let v: Vec<u8> = vec![0; 1048576];
            black_box(v);
        });
    });

    group.finish();
}

// ============================================================================
// Command Line Parsing Benchmarks
// ============================================================================

fn bench_command_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("CommandParsing");

    let commands = vec![
        "play",
        "pause",
        "next",
        "previous",
        "volume 75",
        "seek 120",
        "play something energetic for coding",
        "search artist:Radiohead album:\"OK Computer\"",
    ];

    group.bench_function("parse_simple", |b| {
        b.iter(|| {
            for cmd in &commands[..4] {
                let parts: Vec<&str> = cmd.split_whitespace().collect();
                black_box(parts);
            }
        });
    });

    group.bench_function("parse_complex", |b| {
        b.iter(|| {
            for cmd in &commands[4..] {
                let parts: Vec<&str> = cmd.split_whitespace().collect();
                black_box(parts);
            }
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_fft,
    bench_spectrum_analysis,
    bench_cache_operations,
    bench_json_operations,
    bench_string_operations,
    bench_hashmap_operations,
    bench_allocations,
    bench_command_parsing,
);

criterion_main!(benches);
