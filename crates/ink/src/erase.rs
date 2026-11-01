//! Hit testing for the erasers and the rectangle tool (spec ANNO-6, ANNO-7, ANNO-12).

use std::collections::HashMap;

use scorecraft_doc::{InkPoint, Nib, Stroke};
use scorecraft_geom::{Point, Rect};

use crate::{MAX_POINTS, PressureCurve, Sample, smooth};

/// The most eraser positions one gesture is tested at after densifying.
const MAX_PATH: usize = 100_000;
/// The most samples a stroke is densified to before the pixel eraser cuts it.
const MAX_DENSE: usize = 200_000;

/// What the pixel eraser did to a stroke.
#[derive(Debug, Clone, PartialEq)]
pub enum Erased {
    /// The eraser didn't touch it.
    Untouched,
    /// Every part was erased.
    Removed,
    /// What remains, as one or more strokes with the original's style.
    Split(Vec<Stroke>),
}

/// The eraser's path with extra points so no two are further apart than half the radius, so a
/// fast swipe can't jump over a thin stroke. Non-finite points are dropped.
fn densify_path(path: &[Point], radius: f64) -> Vec<Point> {
    let step = (radius / 2.0).max(0.05);
    let mut out: Vec<Point> = Vec::with_capacity(path.len());
    for p in path.iter().filter(|p| p.is_finite()) {
        if let Some(last) = out.last().copied() {
            let d = last.distance(*p);
            let pieces = (d / step).ceil().min(MAX_PATH as f64) as usize;
            for k in 1..pieces {
                if out.len() >= MAX_PATH {
                    break;
                }
                out.push(last.lerp(*p, k as f64 / pieces as f64));
            }
        }
        if out.len() >= MAX_PATH {
            break;
        }
        out.push(*p);
    }
    out
}

/// Closed-rectangle overlap: a path that is a single point or a straight line has a box with
/// no area, which [`Rect::overlaps`] would miss.
fn meets(a: Rect, b: Rect) -> bool {
    a.x0 <= b.x1 && b.x0 <= a.x1 && a.y0 <= b.y1 && b.y0 <= a.y1
}

/// Closed-rectangle containment.
fn holds(r: Rect, p: Point) -> bool {
    p.x >= r.x0 && p.x <= r.x1 && p.y >= r.y0 && p.y <= r.y1
}

/// Distance from `p` to segment `a`–`b` and where along it (0..=1) the nearest point is.
fn segment_distance(p: Point, a: Point, b: Point) -> (f64, f64) {
    let ab = b - a;
    let len2 = ab.dot(ab);
    let t = if len2 > 0.0 {
        ((p - a).dot(ab) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (p.distance(a.lerp(b, t)), t)
}

/// Eraser positions bucketed by cell, so each test looks at nearby positions only.
struct PathGrid {
    cell: f64,
    cells: HashMap<(i64, i64), Vec<Point>>,
    all: Vec<Point>,
}

/// A query box covering more cells than this scans every position instead.
const MAX_QUERY_CELLS: i64 = 4096;

impl PathGrid {
    fn new(path: Vec<Point>, cell: f64) -> PathGrid {
        let cell = if cell.is_finite() { cell.max(0.5) } else { 0.5 };
        let mut cells: HashMap<(i64, i64), Vec<Point>> = HashMap::new();
        for p in &path {
            cells.entry(key(*p, cell)).or_default().push(*p);
        }
        PathGrid {
            cell,
            cells,
            all: path,
        }
    }

    /// Calls `f` on every position in `b` (and possibly a few just outside); stops when it
    /// returns true.
    fn any_in(&self, b: Rect, mut f: impl FnMut(Point) -> bool) -> bool {
        let (k0, k1) = (
            key(Point::new(b.x0, b.y0), self.cell),
            key(Point::new(b.x1, b.y1), self.cell),
        );
        let span = (k1.0.saturating_sub(k0.0).saturating_add(1))
            .saturating_mul(k1.1.saturating_sub(k0.1).saturating_add(1));
        if !(0..=MAX_QUERY_CELLS).contains(&span) {
            return self.all.iter().filter(|p| holds(b, **p)).any(|p| f(*p));
        }
        for x in k0.0..=k1.0 {
            for y in k0.1..=k1.1 {
                if let Some(ps) = self.cells.get(&(x, y))
                    && ps.iter().any(|p| f(*p))
                {
                    return true;
                }
            }
        }
        false
    }
}

fn key(p: Point, cell: f64) -> (i64, i64) {
    // `as` saturates, so a far-off coordinate lands in an edge cell rather than overflowing.
    ((p.x / cell).floor() as i64, (p.y / cell).floor() as i64)
}

/// Whether any eraser position, as a disc of `radius`, touches the drawn shape of `samples`.
fn shape_touched(samples: &[Sample], grid: &PathGrid, radius: f64) -> bool {
    match samples {
        [] => false,
        [only] => {
            let r = radius + only.half;
            let b = Rect::new(only.p.x, only.p.y, only.p.x, only.p.y).inflate(r);
            grid.any_in(b, |p| p.distance(only.p) <= r)
        }
        _ => samples.windows(2).any(|w| {
            let [a, b] = w else {
                return false;
            };
            let reach = radius + a.half.max(b.half);
            let area = Rect::from_points(a.p, b.p).inflate(reach);
            grid.any_in(area, |p| {
                let (d, t) = segment_distance(p, a.p, b.p);
                d <= radius + a.half + (b.half - a.half) * t
            })
        }),
    }
}

/// Whether the stroke eraser, swept along `path` (page space) with `radius`, touches `stroke`'s
/// drawn shape (spec ANNO-6).
pub fn touches(stroke: &Stroke, path: &[Point], radius: f64, curve: &PressureCurve) -> bool {
    let radius = if radius.is_finite() {
        radius.max(0.0)
    } else {
        0.0
    };
    let path = densify_path(path, radius);
    let Some(path_box) = Rect::bounding(path.iter().copied()) else {
        return false;
    };
    let Some(sb) = stroke.bounds() else {
        return false;
    };
    // Pressure only narrows a stroke, so its nominal bounds are an outer bound.
    if !meets(sb.inflate(radius), path_box) {
        return false;
    }
    let samples = smooth(stroke, curve);
    let width = f64::from(if stroke.width.is_finite() {
        stroke.width.max(0.0)
    } else {
        0.0
    });
    let grid = PathGrid::new(path, radius + width);
    match stroke.nib {
        Nib::Round => shape_touched(&samples, &grid, radius),
        Nib::Chisel => polygons_touched(&crate::outline(stroke, curve).polygons, &grid, radius),
    }
}

/// Whether any eraser position, as a disc of `radius`, touches one of `polys` (convex,
/// counter-clockwise): inside it, or within `radius` of an edge.
fn polygons_touched(polys: &[Vec<Point>], grid: &PathGrid, radius: f64) -> bool {
    polys.iter().any(|poly| {
        let Some(b) = Rect::bounding(poly.iter().copied()) else {
            return false;
        };
        grid.any_in(b.inflate(radius), |p| {
            let mut inside = true;
            let mut near = false;
            for (i, a) in poly.iter().enumerate() {
                let c = poly.get((i + 1) % poly.len()).unwrap_or(a);
                if (*c - *a).cross(p - *a) < 0.0 {
                    inside = false;
                }
                if segment_distance(p, *a, *c).0 <= radius {
                    near = true;
                }
            }
            inside || near
        })
    })
}

/// The stroke's samples with extra ones (pressure and time interpolated) so none are further
/// apart than `step`, capped at [`MAX_DENSE`].
fn densify(points: &[InkPoint], step: f64) -> Vec<InkPoint> {
    let mut out: Vec<InkPoint> = Vec::with_capacity(points.len());
    for p in points.iter().filter(|p| p.x.is_finite() && p.y.is_finite()) {
        if let Some(last) = out.last().copied() {
            let d = f64::from(((p.x - last.x).powi(2) + (p.y - last.y).powi(2)).sqrt());
            let pieces = (d / step).ceil().min(MAX_DENSE as f64) as usize;
            for k in 1..pieces {
                if out.len() >= MAX_DENSE {
                    break;
                }
                let t = k as f32 / pieces as f32;
                out.push(InkPoint {
                    x: last.x + (p.x - last.x) * t,
                    y: last.y + (p.y - last.y) * t,
                    pressure: last.pressure + (p.pressure - last.pressure) * t,
                    tilt: last.tilt + (p.tilt - last.tilt) * t,
                    twist: last.twist + (p.twist - last.twist) * t,
                    t: last.t + ((p.t.saturating_sub(last.t)) as f32 * t) as u32,
                });
            }
        }
        if out.len() >= MAX_DENSE {
            break;
        }
        out.push(*p);
    }
    out
}

/// The pixel eraser (spec ANNO-7): removes the parts of `stroke` within `radius` of `path` (plus
/// the stroke's own half-width there, so what the eraser visibly touched goes) and returns what
/// remains. Pieces keep the stroke's tool, colour and width and their samples' pressure; a piece
/// of a single sample is dropped unless the stroke was a dot; pieces longer than
/// [`MAX_POINTS`] are cut into several.
pub fn erase(stroke: &Stroke, path: &[Point], radius: f64, curve: &PressureCurve) -> Erased {
    if !touches(stroke, path, radius, curve) {
        return Erased::Untouched;
    }
    let radius = if radius.is_finite() {
        radius.max(0.0)
    } else {
        0.0
    };
    let path = densify_path(path, radius);
    let dot = stroke.points.len() == 1;
    let dense = densify(&stroke.points, (radius / 2.0).max(0.5));
    let width = f64::from(if stroke.width.is_finite() {
        stroke.width.max(0.0)
    } else {
        0.0
    });
    let half = |p: &InkPoint| match stroke.tool {
        scorecraft_doc::Tool::Pen => width / 2.0 * f64::from(curve.factor(p.pressure)),
        _ => width / 2.0,
    };
    let grid = PathGrid::new(path, radius + width);
    let erased: Vec<bool> = dense
        .iter()
        .map(|p| {
            let at = Point::new(f64::from(p.x), f64::from(p.y));
            let h = half(p);
            match stroke.nib {
                // The disc of the round nib at this sample.
                Nib::Round => {
                    let r = radius + h;
                    let b = Rect::new(at.x, at.y, at.x, at.y).inflate(r);
                    grid.any_in(b, |q| q.distance(at) <= r)
                }
                // The chisel's upright rectangle at this sample.
                Nib::Chisel => {
                    let nib = Rect::new(
                        at.x - crate::chisel_half_width(h),
                        at.y - h,
                        at.x + crate::chisel_half_width(h),
                        at.y + h,
                    );
                    grid.any_in(nib.inflate(radius), |q| nib.clamp(q).distance(q) <= radius)
                }
            }
        })
        .collect();
    if !erased.iter().any(|e| *e) {
        // The shape was touched between samples only (a thick stroke's edge); remove nothing.
        return Erased::Untouched;
    }
    let mut pieces: Vec<Stroke> = Vec::new();
    let mut run: Vec<InkPoint> = Vec::new();
    let flush = |run: &mut Vec<InkPoint>, pieces: &mut Vec<Stroke>| {
        if run.len() >= 2 || (dot && run.len() == 1) {
            for chunk in chunks(run) {
                pieces.push(Stroke {
                    points: chunk,
                    ..stroke.clone()
                });
            }
        }
        run.clear();
    };
    for (p, gone) in dense.iter().zip(&erased) {
        if *gone {
            flush(&mut run, &mut pieces);
        } else {
            run.push(*p);
        }
    }
    flush(&mut run, &mut pieces);
    if pieces.is_empty() {
        Erased::Removed
    } else {
        Erased::Split(pieces)
    }
}

/// `run` cut into pieces of at most [`MAX_POINTS`] samples that share their end samples, so the
/// line stays unbroken.
fn chunks(run: &[InkPoint]) -> Vec<Vec<InkPoint>> {
    if run.len() <= MAX_POINTS {
        return vec![run.to_vec()];
    }
    let mut out = Vec::new();
    let mut start = 0;
    while start + 1 < run.len() {
        let end = (start + MAX_POINTS).min(run.len());
        if let Some(c) = run.get(start..end) {
            out.push(c.to_vec());
        }
        start = end - 1;
    }
    out
}

/// Whether more than half of `stroke`'s samples lie inside `rect` (spec ANNO-12).
pub fn inside_rect(stroke: &Stroke, rect: Rect) -> bool {
    let finite: Vec<&InkPoint> = stroke
        .points
        .iter()
        .filter(|p| p.x.is_finite() && p.y.is_finite())
        .collect();
    let inside = finite
        .iter()
        .filter(|p| rect.contains(Point::new(f64::from(p.x), f64::from(p.y))))
        .count();
    !finite.is_empty() && inside * 2 > finite.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use scorecraft_doc::{Color, Tool};

    fn line(x0: f32, x1: f32, y: f32) -> Stroke {
        Stroke {
            tool: Tool::Pen,
            color: Color::RED,
            width: 2.0,
            nib: Default::default(),
            points: vec![InkPoint::new(x0, y), InkPoint::new(x1, y)],
        }
    }

    fn c() -> PressureCurve {
        PressureCurve::default()
    }

    #[test]
    fn touches_by_shape_not_samples() {
        // The eraser crosses the middle of a two-sample line: no sample is near it.
        let s = line(0.0, 100.0, 0.0);
        let path = [Point::new(50.0, 20.0), Point::new(50.0, -20.0)];
        assert!(touches(&s, &path, 4.0, &c()));
        let miss = [Point::new(50.0, 20.0), Point::new(50.0, 8.0)];
        assert!(!touches(&s, &miss, 4.0, &c()));
        assert!(!touches(&s, &[], 4.0, &c()));
    }

    #[test]
    fn ink_9_pixel_eraser_splits_in_two() {
        let s = line(0.0, 100.0, 0.0);
        let path = [Point::new(50.0, 20.0), Point::new(50.0, -20.0)];
        let Erased::Split(pieces) = erase(&s, &path, 4.0, &c()) else {
            panic!("expected a split");
        };
        assert_eq!(pieces.len(), 2);
        for p in &pieces {
            assert_eq!((p.tool, p.color, p.width), (s.tool, s.color, s.width));
        }
        let left_end = pieces[0].points.last().unwrap().x;
        let right_start = pieces[1].points[0].x;
        assert!(
            left_end < 46.0 && right_start > 54.0,
            "{left_end} {right_start}"
        );
        assert_eq!(pieces[0].points[0].x, 0.0);
        assert_eq!(pieces[1].points.last().unwrap().x, 100.0);
    }

    #[test]
    fn erasing_everything_removes_and_missing_leaves_alone() {
        let s = line(0.0, 4.0, 0.0);
        let path = [Point::new(-10.0, 0.0), Point::new(14.0, 0.0)];
        assert_eq!(erase(&s, &path, 4.0, &c()), Erased::Removed);
        let far = [Point::new(500.0, 500.0)];
        assert_eq!(erase(&s, &far, 4.0, &c()), Erased::Untouched);
    }

    #[test]
    fn hostile_paths_and_radii() {
        let s = line(0.0, 10.0, 0.0);
        let path = [
            Point::new(f64::NAN, 0.0),
            Point::new(5.0, 0.0),
            Point::new(1e300, 0.0),
        ];
        let _ = erase(&s, &path, f64::INFINITY, &c());
        let _ = erase(&s, &path, -1.0, &c());
        assert!(touches(&s, &[Point::new(5.0, 0.0)], f64::NAN, &c()));
    }

    #[test]
    fn chisel_strokes_are_hit_by_their_real_shape() {
        let mut s = line(0.0, 100.0, 0.0);
        s.width = 30.0;
        s.nib = Nib::Chisel;
        // 15 pt above and below the line, only 5 pt to either side of its ends.
        assert!(touches(&s, &[Point::new(50.0, 14.0)], 0.5, &c()));
        assert!(!touches(&s, &[Point::new(108.0, 0.0)], 0.5, &c()));
        assert!(touches(&s, &[Point::new(104.0, 0.0)], 0.5, &c()));
        let Erased::Split(pieces) = erase(
            &s,
            &[Point::new(50.0, 40.0), Point::new(50.0, -40.0)],
            2.0,
            &c(),
        ) else {
            panic!("expected a split");
        };
        assert_eq!(pieces.len(), 2);
        assert!(pieces.iter().all(|p| p.nib == Nib::Chisel));
    }

    #[test]
    fn rectangle_selection_takes_mostly_inside() {
        let s = Stroke {
            points: vec![
                InkPoint::new(1.0, 1.0),
                InkPoint::new(2.0, 1.0),
                InkPoint::new(50.0, 1.0),
            ],
            ..line(0.0, 0.0, 0.0)
        };
        assert!(inside_rect(&s, Rect::new(0.0, 0.0, 10.0, 10.0)));
        assert!(!inside_rect(&s, Rect::new(40.0, 0.0, 60.0, 10.0)));
    }

    #[test]
    fn long_runs_are_chunked() {
        let run = vec![InkPoint::new(0.0, 0.0); MAX_POINTS * 2 + 5];
        let c = chunks(&run);
        assert!(c.iter().all(|p| p.len() <= MAX_POINTS));
        let total: usize = c.iter().map(Vec::len).sum();
        assert_eq!(total, run.len() + c.len() - 1);
    }
}
