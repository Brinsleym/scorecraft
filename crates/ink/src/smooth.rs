//! Smoothing that keeps corners (spec ANNO-1) and per-sample widths (ANNO-2).
//!
//! A hand-held stylus jitters; handwritten fingerings and accents have sharp corners. So the
//! stroke is first cut at its corners (a direction change sharper than [`CORNER_DEGREES`],
//! measured over a few points of arc so jitter doesn't count), each piece is smoothed on its own
//! with its ends pinned, and then each piece is subdivided along a Catmull–Rom curve so the ink
//! stays round at any zoom.

use scorecraft_doc::{Stroke, Tool};
use scorecraft_geom::Point;

use crate::PressureCurve;

/// A direction change sharper than this, in degrees, is a corner and is not rounded off.
pub const CORNER_DEGREES: f64 = 60.0;
/// The most samples a smoothed stroke has after subdivision; long strokes subdivide less.
pub const MAX_OUTLINE_POINTS: usize = 50_000;
/// Arc length, in points, over which a corner's direction change is measured.
const CORNER_WINDOW: f64 = 3.0;
/// Subdivide so drawn segments are at most this long, in points.
const SEGMENT: f64 = 1.0;
/// The most pieces one segment is cut into.
const MAX_PIECES: usize = 16;
/// Narrowest and widest half-width drawn, in points.
const MIN_HALF: f64 = 0.025;
const MAX_HALF: f64 = 100.0;

/// One point of a smoothed stroke: where, and half the drawn width there.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sample {
    pub p: Point,
    pub half: f64,
}

/// The stroke's centre line, smoothed and subdivided, with the half-width at each point.
/// Non-finite samples (from a damaged file) are skipped. Empty for a stroke with no finite
/// samples.
pub fn smooth(stroke: &Stroke, curve: &PressureCurve) -> Vec<Sample> {
    let raw: Vec<(Point, f64)> = stroke
        .points
        .iter()
        .filter(|p| p.x.is_finite() && p.y.is_finite())
        .map(|p| {
            let pr = if p.pressure.is_finite() {
                f64::from(p.pressure.clamp(0.0, 1.0))
            } else {
                1.0
            };
            (Point::new(f64::from(p.x), f64::from(p.y)), pr)
        })
        .collect();
    let width = if stroke.width.is_finite() && stroke.width > 0.0 {
        f64::from(stroke.width)
    } else {
        MIN_HALF * 2.0
    };
    let half_at = |pressure: f64| {
        let h = match stroke.tool {
            Tool::Pen => width / 2.0 * f64::from(curve.factor(pressure as f32)),
            Tool::Highlighter | Tool::Whiteout => width / 2.0,
        };
        h.clamp(MIN_HALF, MAX_HALF)
    };
    let pts: Vec<Point> = raw.iter().map(|r| r.0).collect();
    let pressure = smooth_pressure(&raw.iter().map(|r| r.1).collect::<Vec<_>>());
    match pts.len() {
        0 => return Vec::new(),
        1 => {
            return pts
                .first()
                .map(|p| Sample {
                    p: *p,
                    half: half_at(pressure.first().copied().unwrap_or(1.0)),
                })
                .into_iter()
                .collect();
        }
        _ => {}
    }
    let corners = corners(&pts);
    // Smooth each run between corners with its ends pinned.
    let mut smoothed = pts.clone();
    for run in corners.windows(2) {
        if let [a, b] = *run {
            smooth_run(&mut smoothed, a, b);
        }
    }
    // Subdivide along a Catmull–Rom curve within each run; corners get no tangent across them.
    let total_len: f64 = smoothed
        .windows(2)
        .map(|w| match w {
            [a, b] => a.distance(*b),
            _ => 0.0,
        })
        .sum();
    let budget = MAX_OUTLINE_POINTS.saturating_sub(smoothed.len()).max(1) as f64;
    let wanted = (total_len / SEGMENT).max(1.0);
    let density = if wanted > budget {
        budget / wanted
    } else {
        1.0
    };
    let mut out: Vec<Sample> = Vec::with_capacity(smoothed.len().min(MAX_OUTLINE_POINTS));
    for run in corners.windows(2) {
        let [a, b] = *run else { continue };
        for i in a..b {
            let (Some(p1), Some(p2)) = (smoothed.get(i), smoothed.get(i + 1)) else {
                continue;
            };
            let p0 = if i > a { smoothed.get(i - 1) } else { None }.unwrap_or(p1);
            let p3 = if i + 1 < b { smoothed.get(i + 2) } else { None }.unwrap_or(p2);
            let (w1, w2) = (
                pressure.get(i).copied().unwrap_or(1.0),
                pressure.get(i + 1).copied().unwrap_or(1.0),
            );
            let len = p1.distance(*p2);
            let pieces = ((len / SEGMENT * density).ceil() as usize).clamp(1, MAX_PIECES);
            if out.is_empty() || i == a && out.last().is_none_or(|s| s.p != *p1) {
                out.push(Sample {
                    p: *p1,
                    half: half_at(w1),
                });
            }
            for k in 1..=pieces {
                let t = k as f64 / pieces as f64;
                let p = if k == pieces {
                    *p2
                } else {
                    catmull_rom(*p0, *p1, *p2, *p3, t)
                };
                if !p.is_finite() || out.len() >= MAX_OUTLINE_POINTS {
                    continue;
                }
                out.push(Sample {
                    p,
                    half: half_at(w1 + (w2 - w1) * t),
                });
            }
        }
    }
    out
}

/// Pressure smoothed twice with 1-2-1 weights; the ends average with their neighbour, so a low
/// first reading doesn't pinch the stroke's start.
fn smooth_pressure(p: &[f64]) -> Vec<f64> {
    let mut v = p.to_vec();
    for _ in 0..2 {
        let prev = v.clone();
        let n = prev.len();
        for (i, slot) in v.iter_mut().enumerate() {
            let at = |j: usize| prev.get(j).copied().unwrap_or(1.0);
            *slot = if n < 2 {
                at(i)
            } else if i == 0 {
                (at(0) + at(1)) / 2.0
            } else if i + 1 == n {
                (at(i) + at(i - 1)) / 2.0
            } else {
                (at(i - 1) + 2.0 * at(i) + at(i + 1)) / 4.0
            };
        }
    }
    v
}

/// Indices that start and end each run: the first sample, every corner, the last sample.
fn corners(pts: &[Point]) -> Vec<usize> {
    let n = pts.len();
    let mut arc = Vec::with_capacity(n);
    let mut s = 0.0;
    for (i, p) in pts.iter().enumerate() {
        if let Some(prev) = i.checked_sub(1).and_then(|j| pts.get(j)) {
            s += prev.distance(*p);
        }
        arc.push(s);
    }
    let at = |i: usize| arc.get(i).copied().unwrap_or(0.0);
    let mut angle = vec![0.0f64; n];
    let (mut back, mut fwd) = (0usize, 0usize);
    for i in 1..n.saturating_sub(1) {
        // Latest sample at least a window behind, earliest at least a window ahead.
        while back + 1 < i && at(i) - at(back + 1) >= CORNER_WINDOW {
            back += 1;
        }
        if fwd < i {
            fwd = i;
        }
        while fwd < n - 1 && at(fwd) - at(i) < CORNER_WINDOW {
            fwd += 1;
        }
        if at(i) - at(back) < CORNER_WINDOW || at(fwd) - at(i) < CORNER_WINDOW {
            continue;
        }
        let (Some(p), Some(b), Some(f)) = (pts.get(i), pts.get(back), pts.get(fwd)) else {
            continue;
        };
        let (u, v) = (*p - *b, *f - *p);
        let (lu, lv) = (u.length(), v.length());
        if lu > 0.0 && lv > 0.0 {
            let c = (u.dot(v) / (lu * lv)).clamp(-1.0, 1.0);
            if let Some(slot) = angle.get_mut(i) {
                *slot = c.acos().to_degrees();
            }
        }
    }
    let mut out = vec![0];
    let mut i = 1;
    while i + 1 < n {
        if angle.get(i).copied().unwrap_or(0.0) > CORNER_DEGREES {
            // The sharpest point of this run of sharp samples is the corner.
            let mut best = i;
            while i + 1 < n && angle.get(i).copied().unwrap_or(0.0) > CORNER_DEGREES {
                if angle.get(i) > angle.get(best) {
                    best = i;
                }
                i += 1;
            }
            out.push(best);
        } else {
            i += 1;
        }
    }
    out.push(n - 1);
    out
}

/// Two 1-2-1 passes over `pts[a..=b]`, keeping `a` and `b` where they are.
fn smooth_run(pts: &mut [Point], a: usize, b: usize) {
    if b <= a + 1 {
        return;
    }
    for _ in 0..2 {
        let Some(prev) = pts.get(a..=b).map(<[Point]>::to_vec) else {
            return;
        };
        for (k, slot) in pts.iter_mut().enumerate().take(b).skip(a + 1) {
            let j = k - a;
            if let (Some(l), Some(m), Some(r)) = (prev.get(j - 1), prev.get(j), prev.get(j + 1)) {
                *slot = Point::new((l.x + 2.0 * m.x + r.x) / 4.0, (l.y + 2.0 * m.y + r.y) / 4.0);
            }
        }
    }
}

fn catmull_rom(p0: Point, p1: Point, p2: Point, p3: Point, t: f64) -> Point {
    let t2 = t * t;
    let t3 = t2 * t;
    let f = |a: f64, b: f64, c: f64, d: f64| {
        0.5 * ((2.0 * b)
            + (-a + c) * t
            + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2
            + (-a + 3.0 * b - 3.0 * c + d) * t3)
    };
    Point::new(f(p0.x, p1.x, p2.x, p3.x), f(p0.y, p1.y, p2.y, p3.y))
}

#[cfg(test)]
mod tests {
    use super::*;
    use scorecraft_doc::{Color, InkPoint};

    fn stroke(tool: Tool, pts: &[(f32, f32, f32)]) -> Stroke {
        Stroke {
            tool,
            color: Color::BLACK,
            width: 2.0,
            nib: Default::default(),
            points: pts
                .iter()
                .map(|&(x, y, p)| InkPoint {
                    pressure: p,
                    ..InkPoint::new(x, y)
                })
                .collect(),
        }
    }

    /// A deterministic jitter in ±amp.
    fn jitter(i: usize, amp: f32) -> f32 {
        let h = (i as u32).wrapping_mul(2_654_435_761) >> 16;
        (h % 1000) as f32 / 1000.0 * 2.0 * amp - amp
    }

    #[test]
    fn ink_15_sharp_corner_survives_jitter() {
        // Right along y = 0 to (20, 0), then up to (20, 20), samples every 0.5 pt, ±0.2 jitter.
        let mut pts = Vec::new();
        for i in 0..=40 {
            pts.push((i as f32 * 0.5, jitter(i, 0.2), 1.0));
        }
        for i in 1..=40 {
            pts.push((20.0 + jitter(i + 100, 0.2), i as f32 * 0.5, 1.0));
        }
        let s = smooth(&stroke(Tool::Pen, &pts), &PressureCurve::default());
        let nearest = s
            .iter()
            .map(|q| q.p.distance(Point::new(20.0, 0.0)))
            .fold(f64::INFINITY, f64::min);
        assert!(nearest < 1.0, "corner rounded off by {nearest}");
    }

    #[test]
    fn jitter_is_reduced_on_a_straight_line() {
        let pts: Vec<(f32, f32, f32)> = (0..200)
            .map(|i| (i as f32 * 0.5, jitter(i, 0.3), 1.0))
            .collect();
        let s = smooth(&stroke(Tool::Pen, &pts), &PressureCurve::default());
        let max_raw = pts.iter().map(|p| p.1.abs()).fold(0.0, f32::max);
        let inner = &s[10..s.len() - 10];
        let max_smooth = inner.iter().map(|q| q.p.y.abs()).fold(0.0, f64::max);
        assert!(
            max_smooth < f64::from(max_raw) * 0.8,
            "{max_smooth} vs {max_raw}"
        );
    }

    #[test]
    fn ink_4_pressure_widens_and_never_below_floor() {
        let pts = [
            (0.0, 0.0, 0.1),
            (10.0, 0.0, 0.1),
            (20.0, 0.0, 1.0),
            (30.0, 0.0, 1.0),
        ];
        let s = smooth(&stroke(Tool::Pen, &pts), &PressureCurve::default());
        let (first, last) = (s[0].half, s[s.len() - 1].half);
        assert!(first < last, "{first} !< {last}");
        for q in &s {
            assert!(q.half >= 0.25 - 1e-9, "{}", q.half);
            assert!(q.half <= 1.0 + 1e-9);
        }
    }

    #[test]
    fn highlighter_ignores_pressure() {
        let pts = [(0.0, 0.0, 0.0), (10.0, 0.0, 1.0)];
        let s = smooth(&stroke(Tool::Highlighter, &pts), &PressureCurve::default());
        assert!(s.iter().all(|q| q.half == 1.0));
    }

    #[test]
    fn subdivides_long_segments_and_keeps_ends() {
        let pts = [(0.0, 0.0, 1.0), (10.0, 10.0, 1.0), (20.0, 0.0, 1.0)];
        let s = smooth(&stroke(Tool::Pen, &pts), &PressureCurve::default());
        assert!(s.len() > 20);
        assert_eq!(s[0].p, Point::new(0.0, 0.0));
        assert_eq!(s[s.len() - 1].p, Point::new(20.0, 0.0));
        for w in s.windows(2) {
            // Pieces follow the chord's length; the curve can be a little longer.
            assert!(w[0].p.distance(w[1].p) <= 1.2);
        }
    }

    #[test]
    fn dot_and_damaged_strokes() {
        let s = smooth(
            &stroke(Tool::Pen, &[(1.0, 2.0, 1.0)]),
            &PressureCurve::default(),
        );
        assert_eq!(s.len(), 1);
        let bad = stroke(
            Tool::Pen,
            &[(f32::NAN, 0.0, 1.0), (f32::INFINITY, 1.0, 1.0)],
        );
        assert!(smooth(&bad, &PressureCurve::default()).is_empty());
        let mut zero = stroke(Tool::Pen, &[(0.0, 0.0, 1.0), (5.0, 0.0, 1.0)]);
        zero.width = f32::NAN;
        assert!(
            smooth(&zero, &PressureCurve::default())
                .iter()
                .all(|q| q.half > 0.0)
        );
    }

    #[test]
    fn huge_strokes_stay_under_the_cap() {
        let pts: Vec<(f32, f32, f32)> = (0..10_000)
            .map(|i| (i as f32 * 50.0, ((i % 2) as f32) * 50.0, 1.0))
            .collect();
        let s = smooth(&stroke(Tool::Pen, &pts), &PressureCurve::default());
        assert!(s.len() <= MAX_OUTLINE_POINTS, "{}", s.len());
    }
}
