//! Audio module
//!
//! Contains the audio playback engine and related utilities.

pub mod analyzer;
mod engine;

pub use analyzer::{FftSize, FrequencyBand, SpectrumAnalyzer, SpectrumResult, WindowFunction};
pub use engine::AudioEngine;
