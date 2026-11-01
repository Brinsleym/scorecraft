//! Snapping the half-page split to the gap between systems (READ-5, gap G2): a page's rows of
//! ink, from a small render, show where the systems are; a split that would cut through one
//! moves to the nearest blank band between them.

use crate::SPLIT_RANGE;

/// A row is blank when less than this share of it is ink (a staff line or a slur is far more).
const BLANK: f32 = 0.004;
/// The thinnest blank band that counts as a gap, as a share of the page height.
const MIN_GAP: f64 = 0.012;
/// A split moves at most this far (share of the page height) to reach a gap.
pub const MAX_SNAP: f64 = 0.15;

/// The blank bands of a page, as `(top, bottom)` shares of its height, from the share of ink in
/// each row (top row first). Bands touching the top or bottom edge are margins, not gaps.
pub fn system_gaps(row_ink: &[f32]) -> Vec<(f64, f64)> {
    let n = row_ink.len();
    if n < 8 {
        return Vec::new();
    }
    let min_run = ((n as f64 * MIN_GAP).ceil() as usize).max(2);
    let mut out = Vec::new();
    let mut start = None;
    for (i, ink) in row_ink.iter().enumerate() {
        let blank = ink.is_finite() && *ink < BLANK;
        match (blank, start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                if s > 0 && i - s >= min_run {
                    out.push((s as f64 / n as f64, i as f64 / n as f64));
                }
                start = None;
            }
            _ => {}
        }
    }
    out
}

/// `split` moved to the middle of the nearest gap between systems, if one lies within
/// [`MAX_SNAP`] and inside [`SPLIT_RANGE`]; otherwise `split` unchanged.
pub fn snap_split(split: f64, row_ink: &[f32]) -> f64 {
    let (lo, hi) = SPLIT_RANGE;
    system_gaps(row_ink)
        .into_iter()
        .map(|(a, b)| (a + b) / 2.0)
        .filter(|c| (lo..=hi).contains(c) && (c - split).abs() <= MAX_SNAP)
        .min_by(|a, b| (a - split).abs().total_cmp(&(b - split).abs()))
        .unwrap_or(split)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A page of `rows` rows with systems (ink) over the given row ranges.
    fn page(rows: usize, systems: &[(usize, usize)]) -> Vec<f32> {
        (0..rows)
            .map(|r| {
                if systems.iter().any(|(a, b)| (*a..*b).contains(&r)) {
                    0.3
                } else {
                    0.0
                }
            })
            .collect()
    }

    #[test]
    fn perf_14_the_split_moves_to_the_gap_between_systems() {
        // Four systems on a 400-row page, gaps around rows 120, 210 and 300.
        let p = page(400, &[(30, 110), (130, 200), (220, 290), (310, 380)]);
        let gaps = system_gaps(&p);
        assert_eq!(gaps.len(), 3, "{gaps:?}");
        // 0.5 (row 200) cuts the second system's bottom: it moves to the gap at row 210.
        assert!((snap_split(0.5, &p) - 0.525).abs() < 1e-9);
        // Already in a gap: its middle.
        assert!((snap_split(0.29, &p) - 0.3).abs() < 1e-9);
        // No gap near: unchanged.
        let dense = page(400, &[(10, 395)]);
        assert_eq!(snap_split(0.5, &dense), 0.5);
        // Margins aren't gaps.
        let one = page(400, &[(100, 300)]);
        assert!(system_gaps(&one).is_empty());
        assert_eq!(snap_split(0.5, &[]), 0.5);
        assert_eq!(snap_split(0.5, &[f32::NAN; 50]), 0.5);
    }
}
