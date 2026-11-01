//! Text notes (ANNO-8, INK-33; `docs/spec/ink.md`).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use serde_json::{Value, json};

use scorecraft_doc::{AnnotationKind, PageKey, ScoreId};
use scorecraft_testkit::pdf::{PageSpec, PdfBuilder};

use crate::Session;

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    s.dispatch(id, &p)
        .unwrap_or_else(|e| panic!("{id} {p}: {e} ({})", e.code()))
}

fn fails(s: &mut Session, id: &str, p: Value) -> String {
    s.dispatch(id, &p).unwrap_err().code().to_owned()
}

fn setup(name: &str) -> (Session, String) {
    let dir = std::env::temp_dir().join(format!("scorecraft-text-{name}-{}", std::process::id()));
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

fn rect(v: &Value) -> [f64; 4] {
    [0, 1, 2, 3].map(|i| v["rect"][i].as_f64().unwrap())
}

#[test]
fn ink_33_long_notes_wrap_and_notes_stay_inside_the_page() {
    let (mut s, id) = setup("wrap");
    run(&mut s, "annotate.tool.text", json!({}));
    let long = "Sample Text ".repeat(40);
    let r = run(
        &mut s,
        "annotate.text.add",
        json!({ "text": long, "x": 0.0, "y": 792.0, "space": "page", "page": 1 }),
    );
    let [x0, y0, x1, y1] = rect(&r);
    assert_eq!(
        (x0, y1),
        (0.0, 792.0),
        "starts at the page's top-left corner"
    );
    assert!(
        x1 <= 612.0 * 0.86 + 1e-6 && x1 > 400.0,
        "wraps at 86%: {x1}"
    );
    assert!(y1 - y0 > 18.0 * 3.0, "several 18 pt lines");
    // Near the right edge: moved left so it ends at the edge.
    let r = run(
        &mut s,
        "annotate.text.add",
        json!({ "text": "Sample Text Sample Text", "x": 592.0, "y": 300.0, "space": "page", "page": 1 }),
    );
    let [x0, _, x1, y1] = rect(&r);
    assert!(
        (x1 - 612.0).abs() < 1e-6 && x0 < 592.0 && y1 == 300.0,
        "{x0} {x1}"
    );
    let sid: ScoreId = id.parse().unwrap();
    let anns = s.library().unwrap().scores[&sid]
        .annotations_on(PageKey(0))
        .to_vec();
    assert_eq!(anns.len(), 2);
    let AnnotationKind::Text(t) = &anns[1].kind else {
        panic!("not text")
    };
    assert_eq!((t.style.size, t.style.background), (12.0, None));
}

#[test]
fn edit_restyles_moves_and_empty_deletes() {
    let (mut s, id) = setup("edit");
    run(&mut s, "annotate.tool.text", json!({ "size": 20 }));
    let r = run(
        &mut s,
        "annotate.text.add",
        json!({ "text": "a", "x": 100.0, "y": 500.0, "space": "page", "page": 1 }),
    );
    let note = r["annotation"].as_str().unwrap().to_owned();
    let small = rect(&r);
    let r = run(
        &mut s,
        "annotate.text.edit",
        json!({ "annotation": note, "text": "Allegro", "size": 36, "bold": true, "background": "#ffffff" }),
    );
    let big = rect(&r);
    assert!(big[2] - big[0] > small[2] - small[0]);
    assert_eq!(
        (big[0], big[3]),
        (100.0, 500.0),
        "keeps its top-left corner"
    );
    let r = run(
        &mut s,
        "annotate.text.edit",
        json!({ "annotation": note, "x": 50.0, "y": 700.0, "space": "page" }),
    );
    assert_eq!((rect(&r)[0], rect(&r)[3]), (50.0, 700.0));
    assert_eq!(
        fails(
            &mut s,
            "annotate.text.edit",
            json!({ "annotation": note, "x": 1.0 })
        ),
        "bad_params"
    );
    let r = run(
        &mut s,
        "annotate.text.edit",
        json!({ "annotation": note, "text": "  \n " }),
    );
    assert_eq!(r["deleted"], true);
    let sid: ScoreId = id.parse().unwrap();
    assert!(
        s.library().unwrap().scores[&sid]
            .annotations_on(PageKey(0))
            .is_empty()
    );
    run(&mut s, "edit.undo", json!({ "score": id }));
    assert_eq!(
        s.library().unwrap().scores[&sid]
            .annotations_on(PageKey(0))
            .len(),
        1
    );
}

#[test]
fn text_tool_settings_and_type_with_touch() {
    let (mut s, _) = setup("tool");
    let st = run(&mut s, "annotate.tool.text", json!({}));
    assert_eq!(st["text"]["size"], 12.0);
    assert_eq!(st["text"]["sizes"], json!([8.0, 36.0]));
    assert_eq!(st["touchLabel"], "Type with Touch");
    let st = run(&mut s, "annotate.style.set", json!({ "width": 30 }));
    assert_eq!(st["text"]["size"], 30.0, "the slider is the size");
    assert!(
        s.dispatch("annotate.style.set", &json!({ "width": 40 }))
            .is_err()
    );
    let st = run(&mut s, "annotate.swatch.select", json!({ "index": 2 }));
    assert_eq!(st["text"]["color"], st["penSwatches"][1]);
    let tap = json!({ "text": "x", "x": 100.0, "y": 100.0, "space": "page", "page": 1, "device": "touch" });
    assert_eq!(
        run(&mut s, "annotate.text.add", tap.clone())["reason"],
        "penOnly"
    );
    run(&mut s, "input.pen.only", json!({ "on": false }));
    assert_eq!(run(&mut s, "annotate.text.add", tap)["added"], true);
}

#[test]
fn notes_shown_in_the_reader_and_bad_params() {
    let (mut s, _) = setup("frame");
    let before = s.reader_frame(true).unwrap()[0].bitmap.clone().unwrap();
    run(
        &mut s,
        "annotate.text.add",
        json!({ "text": "Sample Text", "x": 100.0, "y": 700.0, "space": "page", "page": 1 }),
    );
    let after = s.reader_frame(true).unwrap()[0].bitmap.clone().unwrap();
    assert_ne!(before.words, after.words, "the note is drawn on the page");
    for p in [
        json!({ "text": "", "x": 1, "y": 1, "space": "page", "page": 1 }),
        json!({ "text": "x".repeat(5001), "x": 1, "y": 1, "space": "page", "page": 1 }),
        json!({ "text": "x", "x": 1, "space": "page", "page": 1 }),
        json!({ "text": "x", "x": 1e9, "y": 1, "space": "page", "page": 1 }),
        json!({ "text": "x", "x": 1, "y": 1, "space": "page", "page": 9 }),
        json!({ "text": "x", "x": 1, "y": 1, "device": "eraser" }),
        json!({ "text": "x", "x": -50, "y": -50 }),
    ] {
        assert!(s.dispatch("annotate.text.add", &p).is_err(), "{p}");
    }
    run(
        &mut s,
        "annotate.stroke",
        json!({ "space": "page", "page": 1, "points": [[1, 1], [9, 9]] }),
    );
    let list = run(&mut s, "annotate.page.list", json!({}));
    let stroke = list["annotations"][1]["id"].as_str().unwrap().to_owned();
    assert_eq!(
        fails(
            &mut s,
            "annotate.text.edit",
            json!({ "annotation": stroke, "text": "x" })
        ),
        "bad_params",
        "not a note"
    );
    assert!(
        s.dispatch("annotate.tool.text", &json!({ "size": 7 }))
            .is_err()
    );
}
