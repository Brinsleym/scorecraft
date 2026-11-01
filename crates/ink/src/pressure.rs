//! Pressure to width (spec ANNO-2).

/// Maps pen pressure to a fraction of the nominal width:
/// `floor + (1 − floor) × pressure^gamma`, so a light touch still leaves a visible line.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PressureCurve {
    /// Width fraction at zero pressure, 0..=1.
    pub floor: f32,
    /// Response exponent; below 1 widens light strokes sooner.
    pub gamma: f32,
}

impl Default for PressureCurve {
    fn default() -> Self {
        PressureCurve {
            floor: 0.25,
            gamma: 0.6,
        }
    }
}

impl PressureCurve {
    /// The width fraction for `pressure` (clamped to 0..=1; non-finite counts as full). Always in
    /// `floor..=1`, and 1 for a curve with bad numbers, so a damaged setting never hides ink.
    pub fn factor(&self, pressure: f32) -> f32 {
        let floor = if self.floor.is_finite() {
            self.floor.clamp(0.0, 1.0)
        } else {
            return 1.0;
        };
        let gamma = if self.gamma.is_finite() && self.gamma > 0.0 {
            self.gamma.clamp(0.1, 10.0)
        } else {
            return 1.0;
        };
        let p = if pressure.is_finite() {
            pressure.clamp(0.0, 1.0)
        } else {
            1.0
        };
        floor + (1.0 - floor) * p.powf(gamma)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spans_floor_to_full() {
        let c = PressureCurve::default();
        assert_eq!(c.factor(0.0), 0.25);
        assert_eq!(c.factor(1.0), 1.0);
        let mid = c.factor(0.5);
        assert!(
            mid > 0.25 + 0.75 * 0.5,
            "gamma < 1 widens light strokes: {mid}"
        );
        assert_eq!(c.factor(f32::NAN), 1.0);
        assert_eq!(c.factor(-3.0), 0.25);
    }

    #[test]
    fn bad_curves_draw_full_width() {
        for c in [
            PressureCurve {
                floor: f32::NAN,
                gamma: 1.0,
            },
            PressureCurve {
                floor: 0.2,
                gamma: 0.0,
            },
        ] {
            assert_eq!(c.factor(0.1), 1.0);
        }
    }

    #[test]
    fn monotonic() {
        let c = PressureCurve::default();
        let mut last = 0.0;
        for i in 0..=100 {
            let f = c.factor(i as f32 / 100.0);
            assert!(f >= last);
            last = f;
        }
    }
}
