//! The ruler (spec ANNO-17): a straight edge on a page that strokes starting near it follow.

use scorecraft_doc::InkPoint;
use scorecraft_geom::{Point, Vec2};

/// The ruler's angle snaps to a multiple of 45° within this many degrees.
pub const RULER_SNAP_DEGREES: f64 = 2.0;

/// A straight edge through `origin` at `angle_deg` (counter-clockwise from the x axis), in any
/// one coordinate space: the reader uses screen pixels, so the ruler stays put while pages turn.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ruler {
    pub origin: Point,
    pub angle_deg: f64,
}

impl Ruler {
    /// A ruler with its angle normalised to `[0, 180)` and snapped to 0°, 45°, 90° or 135° when
    /// within [`RULER_SNAP_DEGREES`]. `None` for non-finite numbers.
    pub fn new(origin: Point, angle_deg: f64) -> Option<Ruler> {
        if !(origin.is_finite() && angle_deg.is_finite()) {
            return None;
        }
        let mut a = angle_deg.rem_euclid(180.0);
        let nearest = (a / 45.0).round() * 45.0;
        if (a - nearest).abs() <= RULER_SNAP_DEGREES {
            a = nearest.rem_euclid(180.0);
        }
        Some(Ruler {
            origin,
            angle_deg: a,
        })
    }

    fn direction(&self) -> Vec2 {
        let r = self.angle_deg.to_radians();
        Vec2::new(r.cos(), r.sin())
    }

    /// Where `p` lies along the edge, and how far from it.
    fn project(&self, p: Point) -> (f64, f64) {
        let d = self.direction();
        let v = p - self.origin;
        (v.dot(d), v.cross(d).abs())
    }

    /// How far `p` is from the edge.
    pub fn distance(&self, p: Point) -> f64 {
        self.project(p).1
    }

    /// `points` ruled along the edge when the first one is within `reach` of it: a straight
    /// two-sample stroke from the first sample's place on the edge to the last one's, keeping
    /// their pressures and times. `None` when the stroke starts too far away (it stays freehand).
    pub fn rule(&self, points: &[InkPoint], reach: f64) -> Option<Vec<InkPoint>> {
        let first = points.first()?;
        let last = points.last()?;
        let at = |p: &InkPoint| Point::new(f64::from(p.x), f64::from(p.y));
        let (s0, dist) = self.project(at(first));
        if !(dist.is_finite() && reach.is_finite()) || dist > reach {
            return None;
        }
        let (s1, _) = self.project(at(last));
        let d = self.direction();
        let place = |s: f64, p: &InkPoint| {
            let q = self.origin + d * s;
            InkPoint {
                x: q.x as f32,
                y: q.y as f32,
                ..*p
            }
        };
        let a = place(s0, first);
        if points.len() == 1 || (s1 - s0).abs() < f64::from(crate::MIN_SPACING) {
            return Some(vec![a]);
        }
        Some(vec![a, place(s1, last)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ink_27_wobbly_stroke_near_the_edge_is_ruled() {
        let r = Ruler::new(Point::new(0.0, 100.0), 0.0).unwrap();
        let pts: Vec<InkPoint> = (0..50)
            .map(|i| InkPoint::new(10.0 + i as f32 * 4.0, 105.0 + (i % 3) as f32))
            .collect();
        let ruled = r.rule(&pts, 12.0).unwrap();
        assert_eq!(ruled.len(), 2);
        assert!(ruled.iter().all(|p| (p.y - 100.0).abs() < 1e-4));
        assert!((ruled[0].x - 10.0).abs() < 1e-4 && (ruled[1].x - 206.0).abs() < 1e-4);
        let far: Vec<InkPoint> = pts.iter().map(|p| InkPoint::new(p.x, p.y + 50.0)).collect();
        assert!(r.rule(&far, 12.0).is_none());
    }

    #[test]
    fn angles_normalise_and_snap() {
        let p = Point::new(0.0, 0.0);
        assert_eq!(Ruler::new(p, 91.5).unwrap().angle_deg, 90.0);
        assert_eq!(Ruler::new(p, -1.0).unwrap().angle_deg, 0.0);
        assert_eq!(Ruler::new(p, 200.0).unwrap().angle_deg, 20.0);
        assert_eq!(Ruler::new(p, 179.0).unwrap().angle_deg, 0.0);
        assert!(Ruler::new(p, f64::NAN).is_none());
        let diag = Ruler::new(p, 45.0).unwrap();
        let ruled = diag
            .rule(&[InkPoint::new(1.0, 1.0), InkPoint::new(10.0, 12.0)], 12.0)
            .unwrap();
        assert!((ruled[1].x - ruled[1].y).abs() < 1e-4);
        assert!(diag.rule(&[], 12.0).is_none());
    }
}
