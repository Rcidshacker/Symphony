//! Audio module
//!
//! Contains the audio playback engine and related utilities.

pub mod analyzer;
pub mod features;
mod engine;
mod symphonia_source;
pub mod tap;

pub use analyzer::{
    get_frequency_bands, FftSize, FrequencyBand, SpectrumAnalyzer, SpectrumResult, WindowFunction,
};
pub use engine::AudioEngine;
