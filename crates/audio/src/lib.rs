//! scorecraft-audio: metronome synthesis (spec `docs/spec/performance.md`, PRAC-1); decode,
//! time-stretch, pitch-shift and loops come later.
//!
//! Layer L2 in the workspace map (`docs/architecture.md` §2–3). Pure: [`ClickTrack`] fills
//! sample buffers, sample-accurately, for whatever audio output `scorecraft-devices` opens.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

/// The crate's package name, used by `cargo xtask layers` smoke tests and diagnostics.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

/// A click's length.
const CLICK_MS: f64 = 30.0;
/// The click's pitch on the first beat of the bar, and on the others.
const ACCENT_HZ: f64 = 1_760.0;
const BEAT_HZ: f64 = 1_100.0;

/// One click: a short sine burst with a fast attack and an exponential decay, at most 1.0.
pub fn click(sample_rate: u32, accent: bool) -> Vec<f32> {
    let sr = f64::from(sample_rate.max(1));
    let n = (sr * CLICK_MS / 1000.0) as usize;
    let hz = if accent { ACCENT_HZ } else { BEAT_HZ };
    let gain = if accent { 0.9 } else { 0.6 };
    (0..n)
        .map(|i| {
            let t = i as f64 / sr;
            let attack = (i as f64 / (sr * 0.001)).min(1.0);
            let decay = (-t * 150.0).exp();
            ((2.0 * std::f64::consts::PI * hz * t).sin() * attack * decay * gain) as f32
        })
        .collect()
}

/// A metronome's sound: clicks at `bpm`, the first of every `beats` accented, from the first
/// sample after [`ClickTrack::start`].
#[derive(Debug, Clone)]
pub struct ClickTrack {
    sample_rate: u32,
    bpm: f64,
    beats: u8,
    subdivision: u8,
    accent_on: bool,
    running: bool,
    /// Samples since the track started.
    at: u64,
    accent: Vec<f32>,
    beat: Vec<f32>,
}

impl ClickTrack {
    pub fn new(sample_rate: u32) -> ClickTrack {
        ClickTrack {
            sample_rate: sample_rate.max(1),
            bpm: 100.0,
            beats: 4,
            subdivision: 1,
            accent_on: true,
            running: false,
            at: 0,
            accent: click(sample_rate, true),
            beat: click(sample_rate, false),
        }
    }

    /// Starts (from beat 1) or stops; while running, a new tempo or meter takes effect at once.
    pub fn set(&mut self, running: bool, bpm: f64, beats: u8) {
        if running && !self.running {
            self.at = 0;
        }
        self.running = running;
        if bpm.is_finite() && bpm > 0.0 {
            self.bpm = bpm.clamp(1.0, 1000.0);
        }
        self.beats = beats.max(1);
    }

    pub fn running(&self) -> bool {
        self.running
    }

    /// Clicks per beat (1–4; the extra ones softer) and whether the bar's first beat is
    /// accented (PRAC-2).
    pub fn set_feel(&mut self, subdivision: u8, accent: bool) {
        self.subdivision = subdivision.clamp(1, 4);
        self.accent_on = accent;
    }

    /// Fills `out` (interleaved, `channels` per frame) with the next frames: clicks or silence.
    pub fn fill(&mut self, out: &mut [f32], channels: usize) {
        let channels = channels.max(1);
        let per_beat = ((f64::from(self.sample_rate) * 60.0 / self.bpm) as u64).max(1);
        for frame in out.chunks_mut(channels) {
            let v = if self.running {
                let beat = self.at / per_beat;
                let pos = self.at % per_beat;
                let per_sub = (per_beat / u64::from(self.subdivision.max(1))).max(1);
                let (sub, sub_pos) = (pos / per_sub, (pos % per_sub) as usize);
                self.at += 1;
                if sub == 0 {
                    let table = if self.accent_on && beat.is_multiple_of(u64::from(self.beats)) {
                        &self.accent
                    } else {
                        &self.beat
                    };
                    table.get(sub_pos).copied().unwrap_or(0.0)
                } else if sub < u64::from(self.subdivision) {
                    // Subdivisions: the beat's click, softer.
                    self.beat.get(sub_pos).copied().unwrap_or(0.0) * 0.45
                } else {
                    0.0
                }
            } else {
                0.0
            };
            frame.fill(v);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_matches_package() {
        assert_eq!(super::CRATE_NAME, "scorecraft-audio");
    }

    #[test]
    fn perf_13_clicks_fall_on_the_beat_with_the_first_accented() {
        let mut t = ClickTrack::new(48_000);
        t.set(true, 120.0, 3);
        // Two seconds, stereo: four beats at 24,000 frames each.
        let mut buf = vec![0.0f32; 48_000 * 2 * 2];
        t.fill(&mut buf, 2);
        let frame = |i: usize| buf[i * 2];
        let loud = |from: usize| {
            (from..from + 1440)
                .map(|i| frame(i).abs())
                .fold(0.0, f32::max)
        };
        let quiet = |from: usize| {
            (from..from + 1000)
                .map(|i| frame(i).abs())
                .fold(0.0, f32::max)
        };
        assert!(loud(0) > 0.5, "beat 1 is accented");
        assert!(loud(24_000) > 0.2 && loud(24_000) < loud(0));
        assert!(loud(72_000) > 0.5, "beat 4 starts the next bar of 3");
        assert_eq!(quiet(10_000), 0.0, "silence between clicks");
        assert_eq!(buf[0], buf[1], "every channel the same");
        t.set(false, 120.0, 3);
        let mut more = vec![1.0f32; 64];
        t.fill(&mut more, 2);
        assert!(more.iter().all(|v| *v == 0.0));
    }

    #[test]
    fn subdivisions_click_softly_between_beats() {
        let mut t = ClickTrack::new(48_000);
        t.set(true, 60.0, 4);
        t.set_feel(2, false);
        let mut buf = vec![0.0f32; 48_000];
        t.fill(&mut buf, 1);
        let peak = |from: usize| {
            buf[from..from + 1440]
                .iter()
                .map(|v| v.abs())
                .fold(0.0, f32::max)
        };
        assert!(
            peak(0) > 0.4 && peak(0) < 0.7,
            "no accent: an ordinary beat"
        );
        assert!(
            peak(24_000) > 0.1 && peak(24_000) < peak(0),
            "the half-beat, softer"
        );
        assert_eq!(peak(12_000), 0.0);
    }

    #[test]
    fn clicks_are_short_and_bounded() {
        let c = click(44_100, true);
        assert_eq!(c.len(), 1_323);
        assert!(c.iter().all(|v| v.abs() <= 1.0));
        let mut t = ClickTrack::new(0);
        t.set(true, f64::NAN, 0);
        let mut buf = vec![0.0f32; 16];
        t.fill(&mut buf, 0);
    }
}
