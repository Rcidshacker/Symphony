//! Symphonia-backed decoder for containers rodio 0.19 cannot open (M4A/MP4).
//!
//! rodio's own MP4 path panics while initializing ("Seek errors should not occur"),
//! so M4A files are decoded here and handed to the sink like any other source.

use std::fs::File;
use std::path::Path;
use std::time::Duration;

use rodio::Source;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{Decoder, DecoderOptions};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::{FormatOptions, FormatReader};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// Decodes the default audio track of a file to interleaved `i16` samples
pub struct SymphoniaSource {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn Decoder>,
    track_id: u32,
    channels: u16,
    sample_rate: u32,
    total: Option<Duration>,
    buffer: Vec<i16>,
    position: usize,
}

impl SymphoniaSource {
    /// Open `path`; the error is a short human-readable reason
    pub fn open(path: &Path) -> Result<Self, String> {
        let file = File::open(path).map_err(|e| e.to_string())?;
        let stream = MediaSourceStream::new(Box::new(file), Default::default());

        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }

        let probed = symphonia::default::get_probe()
            .format(
                &hint,
                stream,
                &FormatOptions::default(),
                &MetadataOptions::default(),
            )
            .map_err(|e| e.to_string())?;
        let format = probed.format;

        let track = format.default_track().ok_or("no audio track")?;
        let params = track.codec_params.clone();
        let track_id = track.id;

        let decoder = symphonia::default::get_codecs()
            .make(&params, &DecoderOptions::default())
            .map_err(|e| e.to_string())?;

        let total = match (params.time_base, params.n_frames) {
            (Some(base), Some(frames)) => {
                let time = base.calc_time(frames);
                Some(Duration::from_secs_f64(time.seconds as f64 + time.frac))
            }
            _ => None,
        };

        Ok(Self {
            format,
            decoder,
            track_id,
            channels: params.channels.map_or(2, |c| c.count() as u16).max(1),
            sample_rate: params.sample_rate.unwrap_or(44_100),
            total,
            buffer: Vec::new(),
            position: 0,
        })
    }

    /// Decode the next packet into `buffer`; `false` at the end of the stream
    fn refill(&mut self) -> bool {
        loop {
            let Ok(packet) = self.format.next_packet() else {
                return false;
            };
            if packet.track_id() != self.track_id {
                continue;
            }
            match self.decoder.decode(&packet) {
                Ok(decoded) => {
                    if decoded.frames() == 0 {
                        continue;
                    }
                    let mut samples =
                        SampleBuffer::<i16>::new(decoded.capacity() as u64, *decoded.spec());
                    samples.copy_interleaved_ref(decoded);
                    self.buffer.clear();
                    self.buffer.extend_from_slice(samples.samples());
                    self.position = 0;
                    return true;
                }
                // a corrupt packet: skip it and keep playing
                Err(SymphoniaError::DecodeError(_)) => continue,
                Err(_) => return false,
            }
        }
    }
}

impl Iterator for SymphoniaSource {
    type Item = i16;

    fn next(&mut self) -> Option<i16> {
        if self.position >= self.buffer.len() && !self.refill() {
            return None;
        }
        let sample = self.buffer[self.position];
        self.position += 1;
        Some(sample)
    }
}

impl Source for SymphoniaSource {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> u16 {
        self.channels
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        self.total
    }
}
