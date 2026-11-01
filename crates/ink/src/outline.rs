//! The filled shape of a stroke (spec ANNO-1, ANNO-3, ANNO-5).
//!
//! A stroke's outline is a set of small convex polygons: one quadrilateral per segment of the
//! smoothed centre line (its two ends offset by their own half-widths), plus a disc where the
//! line bends and at round ends. All of them wind the same way, so filling them together with
//! the non-zero rule gives their union: a highlighter that crosses itself covers each pixel once
//! (no build-up), and a renderer needs nothing but a polygon filler. Convex pieces also triangulate
//! as fans for GPU drawing.

use scorecraft_doc::{Nib, Stroke};
use scorecraft_geom::{Point, Rect, Vec2};

use crate::{PressureCurve, Sample, smooth};

/// Disc edges are at most this long, in points.
const DISC_EDGE: f64 = 0.5;
const DISC_MIN_SIDES: usize = 8;
const DISC_MAX_SIDES: usize = 48;
/// A bend sharper than this, in radians, gets a disc to fill the wedge between segments.
const JOIN_ANGLE: f64 = 0.05;

/// A stroke's filled shape in page space: convex polygons, all wound counter-clockwise (y up).
/// Fill them together with the non-zero rule.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Outline {
    pub polygons: Vec<Vec<Point>>,
    /// Bounds of every polygon; `None` when there are none.
    pub bounds: Option<Rect>,
}

impl Outline {
    pub fn is_empty(&self) -> bool {
        self.polygons.is_empty()
    }

    /// Triangles covering the outline, as fans of its convex polygons (for GPU drawing).
    pub fn triangles(&self) -> Vec<[Point; 3]> {
        let mut out = Vec::new();
        for poly in &self.polygons {
            if let Some((first, rest)) = poly.split_first() {
                for w in rest.windows(2) {
                    if let [b, c] = w {
                        out.push([*first, *b, *c]);
                    }
                }
            }
        }
        out
    }

    fn push(&mut self, mut poly: Vec<Point>) {
        if poly.len() < 3 || !poly.iter().all(|p| p.is_finite()) {
            return;
        }
        let area = signed_area(&poly);
        if area.abs() < 1e-12 {
            return;
        }
        if area < 0.0 {
            poly.reverse();
        }
        if let Some(b) = Rect::bounding(poly.iter().copied()) {
            self.bounds = Some(self.bounds.map_or(b, |a| a.union(b)));
        }
        self.polygons.push(poly);
    }
}

/// The outline of `stroke` with pressure mapped by `curve`. A round nib (pen, whiteout, a
/// highlighter with round ends) has round ends and joins; a chisel (the highlighter's default) is
/// an upright rectangle a third as wide as it is tall swept along the stroke, so its ends are cut
/// vertically and a tap is the rectangle itself (spec ANNO-3).
pub fn outline(stroke: &Stroke, curve: &PressureCurve) -> Outline {
    let samples = smooth(stroke, curve);
    match stroke.nib {
        Nib::Round => build_round(&samples),
        Nib::Chisel => build_chisel(&samples),
    }
}

/// Half the chisel's width for a half-height `half`: the nib is a third as wide as it is tall.
pub fn chisel_half_width(half: f64) -> f64 {
    half / 3.0
}

fn chisel_corners(s: &Sample) -> [Point; 4] {
    let (hw, hh) = (chisel_half_width(s.half), s.half);
    let c = s.p;
    [
        Point::new(c.x - hw, c.y - hh),
        Point::new(c.x + hw, c.y - hh),
        Point::new(c.x + hw, c.y + hh),
        Point::new(c.x - hw, c.y + hh),
    ]
}

fn build_chisel(s: &[Sample]) -> Outline {
    let mut out = Outline::default();
    if let [only] = s {
        out.push(chisel_corners(only).to_vec());
        return out;
    }
    for w in s.windows(2) {
        let [a, b] = w else { continue };
        // The nib swept from one sample to the next is the convex hull of its two positions.
        let mut pts: Vec<Point> = chisel_corners(a).to_vec();
        pts.extend(chisel_corners(b));
        out.push(convex_hull(pts));
    }
    out
}

/// The convex hull of `pts` (monotone chain), counter-clockwise.
fn convex_hull(mut pts: Vec<Point>) -> Vec<Point> {
    pts.retain(|p| p.is_finite());
    pts.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    pts.dedup();
    if pts.len() < 3 {
        return pts;
    }
    let cross = |o: Point, a: Point, b: Point| (a - o).cross(b - o);
    let mut hull: Vec<Point> = Vec::with_capacity(pts.len() * 2);
    for pass in 0..2 {
        let start = hull.len();
        let iter: Box<dyn Iterator<Item = &Point>> = if pass == 0 {
            Box::new(pts.iter())
        } else {
            Box::new(pts.iter().rev())
        };
        for p in iter {
            while hull.len() >= start + 2 {
                let (Some(&o), Some(&a)) = (hull.get(hull.len() - 2), hull.last()) else {
                    break;
                };
                if cross(o, a, *p) <= 0.0 {
                    hull.pop();
                } else {
                    break;
                }
            }
            hull.push(*p);
        }
        // The last point of each half is the first of the other.
        hull.pop();
    }
    hull
}

fn build_round(s: &[Sample]) -> Outline {
    let mut out = Outline::default();
    match s {
        [] => return out,
        [only] => {
            out.push(disc(only.p, only.half));
            return out;
        }
        _ => {}
    }
    let n = s.len();
    for (i, w) in s.windows(2).enumerate() {
        let [a, b] = w else { continue };
        let (a, b) = (*a, *b);
        let d = b.p - a.p;
        let len = d.length();
        if len <= 0.0 {
            continue;
        }
        let normal = Vec2::new(-d.y / len, d.x / len);
        out.push(vec![
            a.p + normal * a.half,
            a.p - normal * a.half,
            b.p - normal * b.half,
            b.p + normal * b.half,
        ]);
        // A disc at the join to the next segment when the line bends there.
        if let Some(c) = s.get(i + 2) {
            let e = c.p - b.p;
            let el = e.length();
            if el > 0.0 {
                let cos = (d.dot(e) / (len * el)).clamp(-1.0, 1.0);
                if cos.acos() > JOIN_ANGLE {
                    out.push(disc(b.p, b.half));
                }
            }
        }
    }
    for end in [s.first(), s.get(n - 1)].into_iter().flatten() {
        out.push(disc(end.p, end.half));
    }
    if out.is_empty() {
        // Every sample at one place: a dot.
        if let Some(first) = s.first() {
            out.push(disc(first.p, first.half));
        }
    }
    out
}

/// A disc as a regular polygon whose edges are at most [`DISC_EDGE`] long.
fn disc(c: Point, r: f64) -> Vec<Point> {
    let sides = ((std::f64::consts::TAU * r / DISC_EDGE).ceil() as usize)
        .clamp(DISC_MIN_SIDES, DISC_MAX_SIDES);
    (0..sides)
        .map(|k| {
            let a = std::f64::consts::TAU * k as f64 / sides as f64;
            Point::new(c.x + r * a.cos(), c.y + r * a.sin())
        })
        .collect()
}

fn signed_area(poly: &[Point]) -> f64 {
    let mut a = 0.0;
    for (i, p) in poly.iter().enumerate() {
        let q = poly.get((i + 1) % poly.len()).unwrap_or(p);
        a += p.x * q.y - q.x * p.y;
    }
    a / 2.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use scorecraft_doc::{Color, InkPoint, Tool};

    fn stroke(tool: Tool, pts: &[(f32, f32)]) -> Stroke {
        Stroke {
            tool,
            color: Color::BLACK,
            width: 4.0,
            nib: Default::default(),
            points: pts.iter().map(|&(x, y)| InkPoint::new(x, y)).collect(),
        }
    }

    #[test]
    fn every_polygon_winds_counter_clockwise() {
        let o = outline(
            &stroke(
                Tool::Pen,
                &[(0.0, 0.0), (10.0, 5.0), (0.0, 10.0), (10.0, 15.0)],
            ),
            &PressureCurve::default(),
        );
        assert!(!o.is_empty());
        for p in &o.polygons {
            assert!(signed_area(p) > 0.0);
        }
    }

    #[test]
    fn round_and_chisel_ends() {
        let pen = outline(
            &stroke(Tool::Pen, &[(0.0, 0.0), (10.0, 0.0)]),
            &PressureCurve::default(),
        );
        let b = pen.bounds.unwrap();
        assert!(
            (b.x0 - -2.0).abs() < 1e-6 && (b.x1 - 12.0).abs() < 1e-6,
            "{b:?}"
        );
        let mut hl = stroke(Tool::Highlighter, &[(0.0, 0.0), (12.0, 0.0)]);
        hl.width = 12.0;
        hl.nib = Nib::Chisel;
        let b = outline(&hl, &PressureCurve::default()).bounds.unwrap();
        // Half-width 2 sideways, half-height 6 up and down; ends cut vertically.
        assert!(
            (b.x0 - -2.0).abs() < 1e-6 && (b.x1 - 14.0).abs() < 1e-6,
            "{b:?}"
        );
        assert!(
            (b.y0 - -6.0).abs() < 1e-6 && (b.y1 - 6.0).abs() < 1e-6,
            "{b:?}"
        );
    }

    #[test]
    fn ink_29_chisel_is_thick_across_and_thin_up_and_down() {
        let area = |pts: &[(f32, f32)]| {
            let mut s = stroke(Tool::Highlighter, pts);
            s.width = 12.0;
            s.nib = Nib::Chisel;
            outline(&s, &PressureCurve::default())
        };
        let across = area(&[(0.0, 0.0), (30.0, 0.0)]).bounds.unwrap();
        let down = area(&[(0.0, 0.0), (0.0, 30.0)]).bounds.unwrap();
        assert!((across.height() - 12.0).abs() < 1e-6);
        assert!(
            (down.width() - 4.0).abs() < 1e-6,
            "three times thinner: {down:?}"
        );
        let tap = area(&[(5.0, 5.0)]);
        assert_eq!(tap.polygons.len(), 1);
        let t = tap.bounds.unwrap();
        assert!((t.width() - 4.0).abs() < 1e-6 && (t.height() - 12.0).abs() < 1e-6);
    }

    #[test]
    fn hull_is_convex_and_counter_clockwise() {
        let h = convex_hull(vec![
            Point::new(0.0, 0.0),
            Point::new(2.0, 0.0),
            Point::new(1.0, 1.0),
            Point::new(2.0, 2.0),
            Point::new(0.0, 2.0),
            Point::new(f64::NAN, 1.0),
        ]);
        assert_eq!(h.len(), 4);
        assert!(signed_area(&h) > 0.0);
    }

    #[test]
    fn triangles_cover_each_polygon() {
        let o = outline(
            &stroke(Tool::Pen, &[(0.0, 0.0), (3.0, 0.0)]),
            &PressureCurve::default(),
        );
        let tri: usize = o.polygons.iter().map(|p| p.len() - 2).sum();
        assert_eq!(o.triangles().len(), tri);
    }
}
