//! Cleaning raw samples into what a stroke stores (spec ANNO-1).

use scorecraft_doc::InkPoint;

use crate::InkError;

/// The most samples one stroke holds (spec ANNO-1). Longer gestures continue as a new stroke.
pub const MAX_POINTS: usize = 10_000;
/// Samples closer than this to the previous kept one are dropped, in points.
pub const MIN_SPACING: f32 = 0.1;
/// The largest coordinate accepted, in points: about 350 m, far past any page.
pub const MAX_COORD: f32 = 1.0e6;

/// The samples a stroke stores: non-finite samples dropped, pressure clamped to 0..=1 (a
/// non-finite pressure counts as full), samples closer than [`MIN_SPACING`] to the previous one
/// dropped (the last sample of the gesture is always kept, so the stroke ends where the pen
/// lifted), times made non-decreasing.
pub fn prepare(points: &[InkPoint]) -> Result<Vec<InkPoint>, InkError> {
    if points.len() > MAX_POINTS {
        return Err(InkError::TooManyPoints {
            count: points.len(),
            max: MAX_POINTS,
        });
    }
    let mut out: Vec<InkPoint> = Vec::with_capacity(points.len());
    let mut last_t = 0u32;
    let finite: Vec<&InkPoint> = points
        .iter()
        .filter(|p| p.x.is_finite() && p.y.is_finite())
        .collect();
    let n = finite.len();
    for (i, p) in finite.into_iter().enumerate() {
        if p.x.abs() > MAX_COORD || p.y.abs() > MAX_COORD {
            return Err(InkError::OutOfRange);
        }
        let mut q = *p;
        q.pressure = if q.pressure.is_finite() {
            q.pressure.clamp(0.0, 1.0)
        } else {
            1.0
        };
        for v in [&mut q.tilt, &mut q.twist] {
            if !v.is_finite() {
                *v = 0.0;
            }
        }
        q.t = q.t.max(last_t);
        last_t = q.t;
        let is_last = i + 1 == n;
        let kept = out.len();
        match out.last_mut() {
            Some(prev) if close(prev, &q) => {
                if is_last && kept > 1 {
                    // Keep where the pen lifted rather than the sample just before it.
                    *prev = q;
                }
            }
            _ => out.push(q),
        }
    }
    if out.is_empty() {
        return Err(InkError::Empty);
    }
    Ok(out)
}

fn close(a: &InkPoint, b: &InkPoint) -> bool {
    let (dx, dy) = (a.x - b.x, a.y - b.y);
    dx * dx + dy * dy < MIN_SPACING * MIN_SPACING
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drops_close_and_non_finite_samples() {
        let pts = [
            InkPoint::new(0.0, 0.0),
            InkPoint::new(0.05, 0.0),
            InkPoint::new(f32::NAN, 1.0),
            InkPoint::new(1.0, 0.0),
            InkPoint::new(1.02, 0.0),
        ];
        let out = prepare(&pts).unwrap();
        let xs: Vec<f32> = out.iter().map(|p| p.x).collect();
        assert_eq!(
            xs,
            vec![0.0, 1.02],
            "the lift point replaces its near neighbour"
        );
    }

    #[test]
    fn clamps_pressure_and_orders_time() {
        let mut a = InkPoint::new(0.0, 0.0);
        a.pressure = 3.0;
        a.t = 50;
        let mut b = InkPoint::new(5.0, 0.0);
        b.pressure = f32::INFINITY;
        b.t = 10;
        b.tilt = f32::NAN;
        let out = prepare(&[a, b]).unwrap();
        assert_eq!(out[0].pressure, 1.0);
        assert_eq!(out[1].pressure, 1.0);
        assert_eq!(out[1].t, 50);
        assert_eq!(out[1].tilt, 0.0);
    }

    #[test]
    fn refuses_empty_huge_and_far_strokes() {
        assert_eq!(prepare(&[]), Err(InkError::Empty));
        assert_eq!(
            prepare(&[InkPoint::new(f32::NAN, 0.0)]),
            Err(InkError::Empty)
        );
        let many = vec![InkPoint::new(0.0, 0.0); MAX_POINTS + 1];
        assert!(matches!(
            prepare(&many),
            Err(InkError::TooManyPoints { .. })
        ));
        assert_eq!(
            prepare(&[InkPoint::new(2.0e6, 0.0)]),
            Err(InkError::OutOfRange)
        );
    }

    #[test]
    fn a_tap_is_one_sample() {
        let out = prepare(&[InkPoint::new(3.0, 4.0), InkPoint::new(3.01, 4.0)]).unwrap();
        assert_eq!(out.len(), 1);
    }
}
