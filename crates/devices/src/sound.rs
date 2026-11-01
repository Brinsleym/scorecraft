//! Audio output for the metronome's click (PRAC-1, spec `docs/spec/performance.md`), behind the
//! `audio` feature (cpal: WASAPI on Windows, Core Audio on macOS). Without it — or with no
//! output device — [`ClickOut::new`] reports why and the app keeps the visual pulse alone.

/// Why there is no click.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoundError(pub String);

impl std::fmt::Display for SoundError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for SoundError {}

#[cfg(feature = "audio")]
mod imp {
    use std::sync::{Arc, Mutex};

    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use scorecraft_audio::ClickTrack;

    use super::SoundError;

    /// The default output device, playing a click track.
    pub struct ClickOut {
        track: Arc<Mutex<ClickTrack>>,
        _stream: cpal::Stream,
    }

    impl ClickOut {
        pub fn new() -> Result<ClickOut, SoundError> {
            let err = |e: &dyn std::fmt::Display| SoundError(e.to_string());
            let host = cpal::default_host();
            let device = host
                .default_output_device()
                .ok_or_else(|| SoundError("no audio output device".into()))?;
            let config = device.default_output_config().map_err(|e| err(&e))?;
            if config.sample_format() != cpal::SampleFormat::F32 {
                return Err(SoundError(format!(
                    "the audio output wants {} samples; only 32-bit float is supported",
                    config.sample_format()
                )));
            }
            let config: cpal::StreamConfig = config.into();
            let channels = usize::from(config.channels);
            let track = Arc::new(Mutex::new(ClickTrack::new(config.sample_rate.0)));
            let feed = track.clone();
            let stream = device
                .build_output_stream(
                    &config,
                    move |data: &mut [f32], _| match feed.lock() {
                        Ok(mut t) => t.fill(data, channels),
                        Err(_) => data.fill(0.0),
                    },
                    |_| {},
                    None,
                )
                .map_err(|e| err(&e))?;
            stream.play().map_err(|e| err(&e))?;
            Ok(ClickOut {
                track,
                _stream: stream,
            })
        }

        /// Clicks at `bpm` in `beats`, or silence with `None`.
        pub fn set(&self, click: Option<(f64, u8)>) {
            if let Ok(mut t) = self.track.lock() {
                match click {
                    Some((bpm, beats)) => t.set(true, bpm, beats),
                    None => t.set(false, 0.0, 1),
                }
            }
        }
    }
}

#[cfg(not(feature = "audio"))]
mod imp {
    use super::SoundError;

    /// No audio output in this build.
    pub struct ClickOut;

    impl ClickOut {
        pub fn new() -> Result<ClickOut, SoundError> {
            Err(SoundError("this build has no audio output".into()))
        }

        pub fn set(&self, _click: Option<(f64, u8)>) {}
    }
}

pub use imp::ClickOut;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn without_audio_the_error_says_so() {
        if cfg!(not(feature = "audio")) {
            assert!(ClickOut::new().is_err());
        }
    }
}
