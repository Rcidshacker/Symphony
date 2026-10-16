//! Audio tap
//!
//! Copies the samples the output device is about to play into a shared buffer so the
//! visualizer analyzes what is actually audible (it stops on pause and follows seeks).

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rodio::Source;

/// Mono samples kept for the analyzer (about 0.37 s at 44.1 kHz)
const MAX_BUFFERED: usize = 16_384;

/// Mono samples handed over to the shared buffer at a time (keeps lock traffic low)
const BATCH: usize = 512;

/// Shared buffer between the audio thread and the UI thread
#[derive(Debug)]
pub struct AudioTap {
    samples: Mutex<Vec<f32>>,
    sample_rate: AtomicU32,
}

impl AudioTap {
    /// Create an empty tap
    pub fn new() -> Self {
        Self {
            samples: Mutex::new(Vec::new()),
            sample_rate: AtomicU32::new(44_100),
        }
    }

    /// Take everything buffered since the last call
    pub fn drain(&self) -> Vec<f32> {
        match self.samples.lock() {
            Ok(mut samples) => std::mem::take(&mut *samples),
            Err(_) => Vec::new(),
        }
    }

    /// Drop buffered samples (new track)
    pub fn clear(&self) {
        if let Ok(mut samples) = self.samples.lock() {
            samples.clear();
        }
    }

    /// Sample rate of the current source
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate.load(Ordering::Relaxed)
    }

    fn push(&self, batch: &[f32]) {
        // The audio thread must never wait on the UI: skip the batch if it is busy
        if let Ok(mut samples) = self.samples.try_lock() {
            samples.extend_from_slice(batch);
            if samples.len() > MAX_BUFFERED {
                let excess = samples.len() - MAX_BUFFERED;
                samples.drain(..excess);
            }
        }
    }
}

impl Default for AudioTap {
    fn default() -> Self {
        Self::new()
    }
}

/// Wraps a source and mirrors its samples (mixed down to mono) into an [`AudioTap`]
pub struct TapSource<S: Source<Item = i16>> {
    inner: S,
    tap: Arc<AudioTap>,
    channels: u16,
    frame_sum: f32,
    frame_len: u16,
    pending: Vec<f32>,
}

impl<S: Source<Item = i16>> TapSource<S> {
    /// Wrap `inner`, feeding `tap`
    pub fn new(inner: S, tap: Arc<AudioTap>) -> Self {
        tap.sample_rate
            .store(inner.sample_rate(), Ordering::Relaxed);
        let channels = inner.channels().max(1);
        Self {
            inner,
            tap,
            channels,
            frame_sum: 0.0,
            frame_len: 0,
            pending: Vec::with_capacity(BATCH),
        }
    }

    fn flush(&mut self) {
        if !self.pending.is_empty() {
            self.tap.push(&self.pending);
            self.pending.clear();
        }
    }
}

impl<S: Source<Item = i16>> Iterator for TapSource<S> {
    type Item = i16;

    fn next(&mut self) -> Option<i16> {
        let Some(sample) = self.inner.next() else {
            self.flush();
            return None;
        };

        self.frame_sum += f32::from(sample) / 32_768.0;
        self.frame_len += 1;
        if self.frame_len >= self.channels {
            self.pending
                .push(self.frame_sum / f32::from(self.frame_len));
            self.frame_sum = 0.0;
            self.frame_len = 0;
            if self.pending.len() >= BATCH {
                self.flush();
            }
        }
        Some(sample)
    }
}

impl<S: Source<Item = i16>> Source for TapSource<S> {
    fn current_frame_len(&self) -> Option<usize> {
        self.inner.current_frame_len()
    }

    fn channels(&self) -> u16 {
        self.inner.channels()
    }

    fn sample_rate(&self) -> u32 {
        self.inner.sample_rate()
    }

    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rodio::buffer::SamplesBuffer;

    fn stereo(samples: Vec<i16>) -> SamplesBuffer<i16> {
        SamplesBuffer::new(2, 8_000, samples)
    }

    #[test]
    fn mirrors_samples_as_mono_and_passes_them_through() {
        let tap = Arc::new(AudioTap::new());
        // two stereo frames: (0.5, -0.5) and (1.0 ~ 32767, 0)
        let source = TapSource::new(stereo(vec![16_384, -16_384, 32_767, 0]), tap.clone());
        assert_eq!(tap.sample_rate(), 8_000);

        let played: Vec<i16> = source.collect();
        assert_eq!(played, vec![16_384, -16_384, 32_767, 0]);

        let mono = tap.drain();
        assert_eq!(mono.len(), 2);
        assert!(
            mono[0].abs() < 1e-6,
            "(0.5 + -0.5) / 2 should be 0, got {}",
            mono[0]
        );
        assert!((mono[1] - 0.5).abs() < 1e-3, "got {}", mono[1]);
        assert!(tap.drain().is_empty(), "drain empties the buffer");
    }

    #[test]
    fn keeps_only_the_most_recent_samples() {
        let tap = Arc::new(AudioTap::new());
        let long = vec![1_000i16; (MAX_BUFFERED + 4_000) * 2];
        let _ = TapSource::new(stereo(long), tap.clone()).count();
        assert_eq!(tap.drain().len(), MAX_BUFFERED);
    }

    #[test]
    fn clear_discards_buffered_samples() {
        let tap = Arc::new(AudioTap::new());
        let _ = TapSource::new(stereo(vec![100; 2_000]), tap.clone()).count();
        tap.clear();
        assert!(tap.drain().is_empty());
    }
}
