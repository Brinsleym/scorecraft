//! Stamps: dynamics and hairpins (ANNO-9; `docs/spec/ink.md`).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use serde_json::{Value, json};

use scorecraft_doc::{AnnotationKind, PageKey, ScoreId};
use scorecraft_testkit::pdf::{PageSpec, PdfBuilder};

use crate::Session;

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    s.dispatch(id, &p)
        .unwrap_or_else(|e| panic!("{id} {p}: {e} ({})", e.code()))
}

fn setup(name: &str) -> (Session, String) {
    let dir = std::env::temp_dir().join(format!("scorecraft-stamp-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("score.pdf");
    std::fs::write(
        &path,
        PdfBuilder::new().pages(2, PageSpec::default()).build(),
    )
    .unwrap();
    let mut s = Session::new();
    s.set_clock(Box::new(|| 1_793_404_800_000));
    s.create_memory_library().unwrap();
    let r = run(
        &mut s,
        "library.import.files",
        json!({ "paths": [path.to_string_lossy()] }),
    );
    let id = r["imported"][0]["score"].as_str().unwrap().to_owned();
    run(&mut s, "score.open", json!({ "score": id }));
    (s, id)
}

fn marks(s: &Session, id: &str) -> Vec<AnnotationKind> {
    let sid: ScoreId = id.parse().unwrap();
    s.library().unwrap().scores[&sid]
        .annotations_on(PageKey(0))
        .iter()
        .map(|a| a.kind.clone())
        .collect()
}

fn at(x: f64, y: f64) -> Value {
    json!({ "x": x, "y": y, "space": "page", "page": 1 })
}

/// Edit params: a handle dragged to (`x`, `y`) on the stamp's own page.
fn drag(annotation: &str, handle: &str, x: f64, y: f64) -> Value {
    json!({ "annotation": annotation, "handle": handle, "x": x, "y": y, "space": "page" })
}

#[test]
fn p_is_the_default_and_dynamics_centre_on_the_tap() {
    let (mut s, id) = setup("dyn");
    let st = run(&mut s, "annotate.tool.stamp", json!({}));
    assert_eq!(st["stamp"]["stamp"], "p");
    assert_eq!(st["stamp"]["stamps"].as_array().unwrap().len(), 8);
    assert_eq!(st["stamp"]["size"], 18.0);
    run(&mut s, "annotate.stamp.place", at(200.0, 300.0));
    run(
        &mut s,
        "annotate.tool.stamp",
        json!({ "stamp": "ff", "size": 36 }),
    );
    run(&mut s, "annotate.stamp.place", at(5.0, 300.0));
    let m = marks(&s, &id);
    let AnnotationKind::Stamp(p) = &m[0] else {
        panic!("not a stamp")
    };
    assert_eq!((p.text.as_str(), p.size), ("p", 18.0));
    let c = p.rect.center();
    assert!((c.x - 200.0).abs() < 1e-6 && (c.y - 300.0).abs() < 1e-6);
    let AnnotationKind::Stamp(ff) = &m[1] else {
        panic!("not a stamp")
    };
    assert_eq!(ff.rect.x0, 0.0, "kept inside the page");
    assert!(ff.rect.width() > p.rect.width());
}

#[test]
fn hairpins_place_stretch_and_move_by_their_handles() {
    let (mut s, id) = setup("pin");
    run(
        &mut s,
        "annotate.tool.stamp",
        json!({ "stamp": "crescendo", "size": 18 }),
    );
    let r = run(&mut s, "annotate.stamp.place", at(100.0, 400.0));
    let pin = r["annotation"].as_str().unwrap().to_owned();
    let AnnotationKind::Hairpin(h) = marks(&s, &id)[0] else {
        panic!("not a hairpin")
    };
    assert!(h.crescendo);
    assert_eq!(
        (h.from.x, h.to.x, h.opening),
        (100.0, 172.0, 6.0),
        "a tap: 72 pt, opening a third of 18"
    );
    // Stretch the end, then move the whole hairpin by its middle.
    let r = run(
        &mut s,
        "annotate.stamp.edit",
        drag(&pin, "end", 250.0, 400.0),
    );
    assert_eq!(r["to"]["x"], 250.0);
    let r = run(
        &mut s,
        "annotate.stamp.edit",
        drag(&pin, "middle", 175.0, 500.0),
    );
    assert_eq!(
        (r["from"]["x"].as_f64(), r["to"]["x"].as_f64()),
        (Some(100.0), Some(250.0))
    );
    assert_eq!(r["from"]["y"], 500.0);
    run(&mut s, "edit.undo", json!({ "score": id }));
    let AnnotationKind::Hairpin(h) = marks(&s, &id)[0] else {
        panic!()
    };
    assert_eq!(h.from.y, 400.0, "undo puts it back");
    // A dragged diminuendo.
    run(
        &mut s,
        "annotate.tool.stamp",
        json!({ "stamp": "diminuendo" }),
    );
    let mut drag = at(50.0, 100.0);
    drag["toX"] = json!(150.0);
    drag["toY"] = json!(100.0);
    run(&mut s, "annotate.stamp.place", drag);
    let AnnotationKind::Hairpin(d) = marks(&s, &id)[1] else {
        panic!()
    };
    assert!(!d.crescendo && d.to.x == 150.0);
}

#[test]
fn stamps_show_in_the_reader_and_bad_params() {
    let (mut s, _) = setup("bad");
    let before = s.reader_frame(true).unwrap()[0].bitmap.clone().unwrap();
    run(&mut s, "annotate.tool.stamp", json!({}));
    let r = run(&mut s, "annotate.stamp.place", at(300.0, 400.0));
    let after = s.reader_frame(true).unwrap()[0].bitmap.clone().unwrap();
    assert_ne!(before.words, after.words);
    let dynamic = r["annotation"].as_str().unwrap().to_owned();
    for (id, p) in [
        ("annotate.tool.stamp", json!({ "stamp": "sfz" })),
        ("annotate.tool.stamp", json!({ "size": 100 })),
        ("annotate.stamp.place", json!({ "x": 1 })),
        (
            "annotate.stamp.place",
            json!({ "x": 1, "y": 1, "device": "eraser" }),
        ),
        ("annotate.stamp.edit", json!({ "annotation": dynamic })),
        (
            "annotate.stamp.edit",
            json!({ "annotation": dynamic, "handle": "end", "x": 1, "y": 1, "space": "page" }),
        ),
        (
            "annotate.stamp.edit",
            json!({ "annotation": dynamic, "handle": "top", "size": 20 }),
        ),
    ] {
        assert!(s.dispatch(id, &p).is_err(), "{id} {p}");
    }
    run(
        &mut s,
        "annotate.tool.stamp",
        json!({ "stamp": "crescendo" }),
    );
    let mut zero = at(10.0, 10.0);
    zero["toX"] = json!(10.2);
    zero["toY"] = json!(10.0);
    assert!(
        s.dispatch("annotate.stamp.place", &zero).is_err(),
        "no length"
    );
    let st = run(&mut s, "annotate.style.set", json!({ "width": 40 }));
    assert_eq!(st["stamp"]["size"], 40.0, "the slider is the stamp size");
    let r = run(&mut s, "input.pen.only", json!({ "on": true }));
    assert_eq!(r["penOnly"], true);
    let mut touch = at(10.0, 10.0);
    touch["device"] = json!("touch");
    assert_eq!(
        run(&mut s, "annotate.stamp.place", touch)["reason"],
        "penOnly"
    );
}
