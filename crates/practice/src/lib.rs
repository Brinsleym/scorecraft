//! scorecraft-practice: metronome scheduling (spec `docs/spec/performance.md`, PRAC-1); the
//! tuner, practice log and reports come later.
//!
//! Layer L2 in the workspace map (`docs/architecture.md` §2–3). Pure: every function takes the
//! time in milliseconds, so the engine's clock drives it and tests are exact.
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

/// The tempo range, beats per minute.
pub const MIN_BPM: f64 = 20.0;
pub const MAX_BPM: f64 = 300.0;
/// Beats per bar the metronome offers.
pub const METERS: [u8; 4] = [2, 3, 4, 6];
/// Taps further apart than this start a new tap tempo.
pub const TAP_RESET_MS: u64 = 2000;
/// Taps averaged: the intervals between the last four.
const TAPS: usize = 4;

/// One beat.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Beat {
    /// Beats since the metronome started, from 0.
    pub count: u64,
    /// Position in the bar, from 0; 0 is accented.
    pub in_bar: u8,
    pub accent: bool,
    /// How far through the beat, 0..1 (0 is the click).
    pub phase: f64,
    /// When the next beat falls, in ms on the caller's clock.
    pub next_ms: u64,
}

/// A metronome: tempo, beats per bar, running or not, and the taps of a tap tempo.
#[derive(Debug, Clone, PartialEq)]
pub struct Metronome {
    bpm: f64,
    beats: u8,
    started_ms: Option<u64>,
    taps: Vec<u64>,
}

impl Default for Metronome {
    fn default() -> Self {
        Metronome {
            bpm: 100.0,
            beats: 4,
            started_ms: None,
            taps: Vec::new(),
        }
    }
}

impl Metronome {
    pub fn new() -> Metronome {
        Metronome::default()
    }

    pub fn bpm(&self) -> f64 {
        self.bpm
    }

    pub fn beats(&self) -> u8 {
        self.beats
    }

    pub fn running(&self) -> bool {
        self.started_ms.is_some()
    }

    /// Sets the tempo, clamped to [`MIN_BPM`]..=[`MAX_BPM`]; not a number is refused. A running
    /// metronome keeps its beat count going from now.
    pub fn set_bpm(&mut self, bpm: f64, now_ms: u64) -> bool {
        if !bpm.is_finite() {
            return false;
        }
        let bpm = bpm.clamp(MIN_BPM, MAX_BPM);
        if let Some(beat) = self.beat_at(now_ms) {
            // Keep the beat in progress: restart the grid at the last click.
            let beat_ms = 60_000.0 / self.bpm;
            let last = now_ms.saturating_sub((beat.phase * beat_ms) as u64);
            self.started_ms =
                Some(last.saturating_sub((beat.count as f64 * 60_000.0 / bpm) as u64));
        }
        self.bpm = bpm;
        true
    }

    /// Sets beats per bar, one of [`METERS`].
    pub fn set_beats(&mut self, beats: u8) -> bool {
        if METERS.contains(&beats) {
            self.beats = beats;
            true
        } else {
            false
        }
    }

    pub fn start(&mut self, now_ms: u64) {
        if self.started_ms.is_none() {
            self.started_ms = Some(now_ms);
        }
    }

    pub fn stop(&mut self) {
        self.started_ms = None;
    }

    /// A tap of the tempo button. With four taps no more than [`TAP_RESET_MS`] apart, the tempo
    /// becomes the average of the last three intervals and is returned.
    pub fn tap(&mut self, now_ms: u64) -> Option<f64> {
        if self
            .taps
            .last()
            .is_some_and(|t| now_ms < *t || now_ms - t > TAP_RESET_MS)
        {
            self.taps.clear();
        }
        self.taps.push(now_ms);
        if self.taps.len() > TAPS {
            self.taps.remove(0);
        }
        if self.taps.len() < TAPS {
            return None;
        }
        let (first, last) = (*self.taps.first()?, *self.taps.last()?);
        let interval = (last - first) as f64 / (TAPS - 1) as f64;
        if interval <= 0.0 {
            return None;
        }
        let bpm = (60_000.0 / interval).clamp(MIN_BPM, MAX_BPM);
        self.set_bpm(bpm, now_ms);
        Some(self.bpm)
    }

    /// The beat at `now_ms`, if running.
    pub fn beat_at(&self, now_ms: u64) -> Option<Beat> {
        let start = self.started_ms?;
        let beat_ms = 60_000.0 / self.bpm;
        let elapsed = now_ms.saturating_sub(start) as f64;
        let count = (elapsed / beat_ms).floor();
        let phase = (elapsed / beat_ms) - count;
        let count = count as u64;
        let in_bar = (count % u64::from(self.beats.max(1))) as u8;
        Some(Beat {
            count,
            in_bar,
            accent: in_bar == 0,
            phase,
            next_ms: start + ((count + 1) as f64 * beat_ms).ceil() as u64,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_matches_package() {
        assert_eq!(super::CRATE_NAME, "scorecraft-practice");
    }

    #[test]
    fn perf_6_beats_fall_on_the_tempo_with_the_first_accented() {
        let mut m = Metronome::new();
        assert!(m.beat_at(0).is_none());
        m.set_bpm(120.0, 0);
        m.set_beats(3);
        m.start(1000);
        let b = m.beat_at(1000).unwrap();
        assert_eq!((b.count, b.in_bar, b.accent, b.next_ms), (0, 0, true, 1500));
        let b = m.beat_at(1750).unwrap();
        assert_eq!((b.count, b.in_bar, b.accent), (1, 1, false));
        assert!((b.phase - 0.5).abs() < 1e-9);
        assert!(
            m.beat_at(2500).unwrap().accent,
            "beat 3 starts the next bar of 3"
        );
        m.stop();
        assert!(m.beat_at(3000).is_none());
        assert!(!m.set_beats(5));
        m.set_bpm(1000.0, 0);
        assert_eq!(m.bpm(), MAX_BPM);
        assert!(!m.set_bpm(f64::NAN, 0));
    }

    #[test]
    fn perf_7_four_taps_set_the_tempo_and_a_pause_starts_again() {
        let mut m = Metronome::new();
        assert_eq!(m.tap(0), None);
        assert_eq!(m.tap(500), None);
        assert_eq!(m.tap(1000), None);
        assert_eq!(m.tap(1500), Some(120.0));
        // A fifth tap averages the last three intervals.
        let bpm = m.tap(2100).unwrap();
        assert!((bpm - 60_000.0 / (1600.0 / 3.0)).abs() < 1e-9, "{bpm}");
        // After a pause the taps start again.
        assert_eq!(m.tap(5000), None);
        assert_eq!(m.tap(5250), None);
    }

    #[test]
    fn changing_tempo_keeps_a_running_beat() {
        let mut m = Metronome::new();
        m.set_bpm(60.0, 0);
        m.start(0);
        m.set_bpm(120.0, 3_250);
        let b = m.beat_at(3_250).unwrap();
        assert_eq!(b.count, 3, "the beat in progress keeps its number");
        assert!(m.beat_at(3_600).unwrap().count >= 3);
    }
}
