//! Shapes (spec ANNO-16): a stroke that is nearly straight becomes a straight line, and a closed
//! stroke that is nearly round becomes an ellipse; anything else stays freehand. The result is an
//! ordinary stroke's samples, so it draws, erases and selects like any other.

use scorecraft_doc::InkPoint;
use scorecraft_geom::{Point, Vec2};

use crate::MAX_POINTS;

/// What a stroke was recognised as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    Line,
    Ellipse,
}

impl Shape {
    pub fn id(self) -> &'static str {
        match self {
            Shape::Line => "line",
            Shape::Ellipse => "ellipse",
        }
    }
}

/// A line may stray from the straight line between its ends by this share of its length...
pub const LINE_TOLERANCE: f64 = 0.06;
/// ...or by this many points, whichever is more (a short line drawn by hand wobbles too).
pub const LINE_SLACK: f64 = 1.5;
/// A path this many times longer than the distance between its ends doubled back on itself and
/// isn't a line.
const LINE_MAX_DETOUR: f64 = 1.2;
/// Shapes smaller than this, in points, stay as drawn (a dot or a tick).
pub const MIN_SHAPE_SIZE: f64 = 6.0;
/// A closed stroke's ends are within this share of its size.
const CLOSE_SHARE: f64 = 0.25;
/// An ellipse's samples stray from it by this share of its radius on average, and by
/// [`ELLIPSE_MAX_ERROR`] at most.
const ELLIPSE_MEAN_ERROR: f64 = 0.12;
const ELLIPSE_MAX_ERROR: f64 = 0.35;
/// The narrowest ellipse recognised, as minor over major axis.
const ELLIPSE_MIN_RATIO: f64 = 0.15;
/// A stroke must go this far round its centre to be an ellipse (315°).
const ELLIPSE_MIN_TURN: f64 = 1.75 * std::f64::consts::PI;
/// Samples along a recognised line, in points apart.
const LINE_STEP: f64 = 2.0;
/// Samples round a recognised ellipse.
const ELLIPSE_POINTS: usize = 96;
/// The path is resampled to this many evenly spaced points before fitting an ellipse, so a
/// slowly drawn stretch doesn't outweigh a fast one.
const FIT_SAMPLES: usize = 128;

fn point(p: &InkPoint) -> Point {
    Point::new(f64::from(p.x), f64::from(p.y))
}

fn cross(a: Vec2, b: Vec2) -> f64 {
    a.x * b.y - a.y * b.x
}

fn path_length(pts: &[Point]) -> f64 {
    pts.iter()
        .zip(pts.iter().skip(1))
        .map(|(a, b)| a.distance(*b))
        .sum()
}

/// `pts` resampled to `n` points evenly spaced along the path.
fn resample(pts: &[Point], n: usize) -> Vec<Point> {
    let total = path_length(pts);
    let (Some(first), true) = (pts.first(), total > 0.0 && n >= 2) else {
        return pts.to_vec();
    };
    let step = total / (n - 1) as f64;
    let mut out = vec![*first];
    let mut want = step;
    let mut walked = 0.0;
    for (a, b) in pts.iter().zip(pts.iter().skip(1)) {
        let seg = a.distance(*b);
        while seg > 0.0 && walked + seg >= want && out.len() < n {
            out.push(a.lerp(*b, (want - walked) / seg));
            want += step;
        }
        walked += seg;
    }
    if out.len() < n
        && let Some(last) = pts.last()
    {
        out.push(*last);
    }
    out
}

/// The mean pressure and the first and last times of `points`.
fn feel(points: &[InkPoint]) -> (f32, u32, u32) {
    let n = points.len().max(1) as f32;
    let pressure = points.iter().map(|p| p.pressure).sum::<f32>() / n;
    let t0 = points.first().map_or(0, |p| p.t);
    let t1 = points.last().map_or(t0, |p| p.t).max(t0);
    (pressure.clamp(0.0, 1.0), t0, t1)
}

fn sample(p: Point, pressure: f32, t0: u32, t1: u32, share: f64) -> InkPoint {
    InkPoint {
        x: p.x as f32,
        y: p.y as f32,
        pressure,
        tilt: 0.0,
        twist: 0.0,
        t: t0 + ((f64::from(t1 - t0)) * share.clamp(0.0, 1.0)) as u32,
    }
}

/// The straight line from the first sample to the last, if `points` is nearly straight.
fn as_line(points: &[InkPoint], pts: &[Point]) -> Option<Vec<InkPoint>> {
    let (a, b) = (*pts.first()?, *pts.last()?);
    let d = a.distance(b);
    if d < MIN_SHAPE_SIZE || path_length(pts) > d * LINE_MAX_DETOUR {
        return None;
    }
    let dir = b - a;
    let worst = pts
        .iter()
        .map(|p| cross(dir, *p - a).abs() / d)
        .fold(0.0, f64::max);
    if worst > (LINE_TOLERANCE * d).max(LINE_SLACK) {
        return None;
    }
    let (pressure, t0, t1) = feel(points);
    let n = ((d / LINE_STEP).ceil() as usize + 1).clamp(2, MAX_POINTS);
    Some(
        (0..n)
            .map(|k| {
                let s = k as f64 / (n - 1) as f64;
                sample(a.lerp(b, s), pressure, t0, t1, s)
            })
            .collect(),
    )
}

/// The ellipse that best fits `points`, if they close on themselves and lie near it.
fn as_ellipse(points: &[InkPoint], pts: &[Point]) -> Option<Vec<InkPoint>> {
    let (first, last) = (*pts.first()?, *pts.last()?);
    let bounds = scorecraft_geom::Rect::bounding(pts.iter().copied())?;
    let size = bounds.width().max(bounds.height());
    if size < MIN_SHAPE_SIZE * 2.0 || first.distance(last) > CLOSE_SHARE * size {
        return None;
    }
    let even = resample(pts, FIT_SAMPLES);
    let n = even.len() as f64;
    let (sx, sy) = even.iter().fold((0.0, 0.0), |(x, y), p| (x + p.x, y + p.y));
    let c = Point::new(sx / n, sy / n);
    let (cxx, cyy, cxy) = even.iter().fold((0.0, 0.0, 0.0), |(xx, yy, xy), p| {
        let (dx, dy) = (p.x - c.x, p.y - c.y);
        (xx + dx * dx, yy + dy * dy, xy + dx * dy)
    });
    let theta = 0.5 * (2.0 * cxy).atan2(cxx - cyy);
    let (e1, e2) = (
        Vec2::new(theta.cos(), theta.sin()),
        Vec2::new(-theta.sin(), theta.cos()),
    );
    let along = |p: &Point| {
        let v = *p - c;
        (v.x * e1.x + v.y * e1.y, v.x * e2.x + v.y * e2.y)
    };
    let (su, sv) = even
        .iter()
        .map(along)
        .fold((0.0, 0.0), |(su, sv), (u, v)| (su + u * u, sv + v * v));
    // For points spread evenly round an ellipse, the mean square along an axis is half its
    // radius squared.
    let (ra, rb) = ((2.0 * su / n).sqrt(), (2.0 * sv / n).sqrt());
    if !(ra.is_finite() && rb.is_finite() && ra > 0.0) || rb < ELLIPSE_MIN_RATIO * ra {
        return None;
    }
    let errors: Vec<f64> = even
        .iter()
        .map(along)
        .map(|(u, v)| ((u / ra).powi(2) + (v / rb).powi(2)).sqrt() - 1.0)
        .map(f64::abs)
        .collect();
    let mean = errors.iter().sum::<f64>() / n;
    let worst = errors.iter().copied().fold(0.0, f64::max);
    if mean > ELLIPSE_MEAN_ERROR || worst > ELLIPSE_MAX_ERROR {
        return None;
    }
    // How far round the centre the stroke goes, and which way.
    let angles: Vec<f64> = even
        .iter()
        .map(along)
        .map(|(u, v)| (v / rb).atan2(u / ra))
        .collect();
    let turn: f64 = angles
        .iter()
        .zip(angles.iter().skip(1))
        .map(|(a, b)| {
            let d = b - a;
            (d + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI
        })
        .sum();
    if turn.abs() < ELLIPSE_MIN_TURN {
        return None;
    }
    let start = *angles.first()?;
    let sign = turn.signum();
    let (pressure, t0, t1) = feel(points);
    Some(
        (0..=ELLIPSE_POINTS)
            .map(|k| {
                let s = k as f64 / ELLIPSE_POINTS as f64;
                let a = start + sign * s * std::f64::consts::TAU;
                let (u, v) = (ra * a.cos(), rb * a.sin());
                let p = c + Vec2::new(u * e1.x + v * e2.x, u * e1.y + v * e2.y);
                sample(p, pressure, t0, t1, s)
            })
            .collect(),
    )
}

/// `points` as a straight line or an ellipse, if they nearly are one; `None` keeps them
/// freehand. Non-finite samples are ignored.
pub fn straighten(points: &[InkPoint]) -> Option<(Shape, Vec<InkPoint>)> {
    let finite: Vec<InkPoint> = points.iter().filter(|p| p.is_finite()).copied().collect();
    if finite.len() < 2 || finite.len() > MAX_POINTS {
        return None;
    }
    let pts: Vec<Point> = finite.iter().map(point).collect();
    if let Some(line) = as_line(&finite, &pts) {
        return Some((Shape::Line, line));
    }
    as_ellipse(&finite, &pts).map(|e| (Shape::Ellipse, e))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pts(v: &[(f64, f64)]) -> Vec<InkPoint> {
        v.iter()
            .map(|(x, y)| InkPoint::new(*x as f32, *y as f32))
            .collect()
    }

    /// A deterministic wobble in [-1, 1].
    fn wobble(k: usize) -> f64 {
        ((k * 7919 % 101) as f64 / 50.0) - 1.0
    }

    #[test]
    fn a_wobbly_straight_stroke_becomes_a_line() {
        let raw: Vec<(f64, f64)> = (0..=50)
            .map(|k| {
                (
                    100.0 + k as f64 * 4.0,
                    300.0 + (k as f64 * 0.7).sin() * 3.0 + wobble(k) * 0.5,
                )
            })
            .collect();
        let (shape, out) = straighten(&pts(&raw)).unwrap();
        assert_eq!(shape, Shape::Line);
        let (a, b) = (out.first().unwrap(), out.last().unwrap());
        assert_eq!((a.x, b.x), (100.0, 300.0));
        // Straight: every sample on the segment between the ends.
        let d = Vec2::new(f64::from(b.x - a.x), f64::from(b.y - a.y));
        for p in &out {
            let v = Vec2::new(f64::from(p.x - a.x), f64::from(p.y - a.y));
            assert!(cross(d, v).abs() / d.length() < 1e-3);
        }
    }

    #[test]
    fn a_rough_circle_becomes_an_ellipse_and_scribbles_stay() {
        let raw: Vec<(f64, f64)> = (0..=80)
            .map(|k| {
                let a = k as f64 / 80.0 * std::f64::consts::TAU * 1.02;
                let r = 40.0 + wobble(k) * 2.5;
                (200.0 + r * 1.5 * a.cos(), 400.0 + r * a.sin())
            })
            .collect();
        let (shape, out) = straighten(&pts(&raw)).unwrap();
        assert_eq!(shape, Shape::Ellipse);
        assert_eq!(out.len(), ELLIPSE_POINTS + 1);
        let xs: Vec<f64> = out.iter().map(|p| f64::from(p.x)).collect();
        let w = xs.iter().copied().fold(f64::MIN, f64::max)
            - xs.iter().copied().fold(f64::MAX, f64::min);
        assert!((w - 120.0).abs() < 10.0, "{w}");
        // A zigzag is neither.
        let zig: Vec<(f64, f64)> = (0..30)
            .map(|k| (k as f64 * 5.0, if k % 2 == 0 { 0.0 } else { 30.0 }))
            .collect();
        assert!(straighten(&pts(&zig)).is_none());
        // An open arc (a slur) stays freehand.
        let arc: Vec<(f64, f64)> = (0..=40)
            .map(|k| {
                let a = k as f64 / 40.0 * std::f64::consts::PI;
                (100.0 - 60.0 * a.cos(), 100.0 + 25.0 * a.sin())
            })
            .collect();
        assert!(straighten(&pts(&arc)).is_none());
    }

    #[test]
    fn dots_and_hostile_input_stay_as_they_are() {
        assert!(straighten(&pts(&[(0.0, 0.0), (2.0, 0.0)])).is_none());
        assert!(straighten(&[]).is_none());
        let mut bad = pts(&[(0.0, 0.0), (50.0, 0.0)]);
        bad.push(InkPoint::new(f32::NAN, 1.0));
        let (shape, _) = straighten(&bad).unwrap();
        assert_eq!(shape, Shape::Line);
        // A closed stroke drawn back and forth on one line is not an ellipse.
        let flat = pts(&[(0.0, 0.0), (60.0, 0.0), (0.0, 0.5)]);
        assert!(straighten(&flat).is_none());
    }
}
