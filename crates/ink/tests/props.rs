//! Property tests: any samples, widths and eraser paths give finite geometry and never panic.

use proptest::prelude::*;
use scorecraft_doc::{Color, InkPoint, Nib, Stroke, Tool};
use scorecraft_geom::Point;
use scorecraft_ink::{
    Erased, MAX_OUTLINE_POINTS, PressureCurve, erase, outline, prepare, smooth, touches,
};

fn any_f32() -> impl Strategy<Value = f32> {
    prop_oneof![
        8 => -2000.0f32..2000.0,
        1 => Just(f32::NAN),
        1 => Just(f32::INFINITY),
        1 => Just(0.0f32),
    ]
}

fn point() -> impl Strategy<Value = InkPoint> {
    (any_f32(), any_f32(), any_f32(), any::<u32>()).prop_map(|(x, y, pressure, t)| InkPoint {
        x,
        y,
        pressure,
        t,
        ..InkPoint::default()
    })
}

fn tool() -> impl Strategy<Value = Tool> {
    prop_oneof![
        Just(Tool::Pen),
        Just(Tool::Highlighter),
        Just(Tool::Whiteout)
    ]
}

fn stroke() -> impl Strategy<Value = Stroke> {
    (
        tool(),
        any_f32(),
        any::<bool>(),
        prop::collection::vec(point(), 0..60),
    )
        .prop_map(|(tool, width, chisel, points)| Stroke {
            tool,
            color: Color::BLACK,
            width,
            nib: if chisel { Nib::Chisel } else { Nib::Round },
            points,
        })
}

proptest! {
    #[test]
    fn outlines_are_finite(s in stroke()) {
        let o = outline(&s, &PressureCurve::default());
        for poly in &o.polygons {
            prop_assert!(poly.len() >= 3);
            prop_assert!(poly.iter().all(|p| p.is_finite()));
        }
        prop_assert!(smooth(&s, &PressureCurve::default()).len() <= MAX_OUTLINE_POINTS);
    }

    #[test]
    fn prepare_is_idempotent(pts in prop::collection::vec(point(), 0..60)) {
        if let Ok(once) = prepare(&pts) {
            let twice = prepare(&once).unwrap();
            prop_assert_eq!(once, twice);
        }
    }

    #[test]
    fn erasing_never_grows_the_ink(s in stroke(), path in prop::collection::vec((any_f32(), any_f32()), 0..10), r in any_f32()) {
        let path: Vec<Point> = path.into_iter().map(|(x, y)| Point::new(f64::from(x), f64::from(y))).collect();
        let r = f64::from(r);
        let hit = touches(&s, &path, r, &PressureCurve::default());
        match erase(&s, &path, r, &PressureCurve::default()) {
            Erased::Untouched => {}
            Erased::Removed => prop_assert!(hit),
            Erased::Split(pieces) => {
                prop_assert!(hit);
                for p in pieces {
                    prop_assert!(!p.points.is_empty());
                    prop_assert!(p.points.len() <= scorecraft_ink::MAX_POINTS);
                    prop_assert_eq!(p.tool, s.tool);
                }
            }
        }
    }
}
