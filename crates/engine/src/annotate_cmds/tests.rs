//! Acceptance tests INK-1..3, INK-8..13 and INK-16..19 (`docs/spec/ink.md`).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::path::PathBuf;

use serde_json::{Value, json};

use scorecraft_doc::{AnnotationKind, PageKey, ScoreId};
use scorecraft_ops::{Op, Transaction};
use scorecraft_testkit::pdf::{PageSpec, PdfBuilder};

use crate::Session;

fn temp_pdf(name: &str, pages: usize, spec: PageSpec) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("scorecraft-ink-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("score.pdf");
    std::fs::write(&path, PdfBuilder::new().pages(pages, spec).build()).unwrap();
    path
}

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    s.dispatch(id, &p)
        .unwrap_or_else(|e| panic!("{id} {p}: {e} ({})", e.code()))
}

fn fails(s: &mut Session, id: &str, p: Value) -> String {
    s.dispatch(id, &p).unwrap_err().code().to_owned()
}

/// A memory library with one imported score of `pages` letter pages, open in the reader.
fn setup(name: &str, pages: usize) -> (Session, String) {
    let mut s = Session::new();
    s.set_clock(Box::new(|| 1_793_404_800_000));
    s.create_memory_library().unwrap();
    let path = temp_pdf(name, pages, PageSpec::default());
    let r = run(
        &mut s,
        "library.import.files",
        json!({ "paths": [path.to_string_lossy()] }),
    );
    let id = r["imported"][0]["score"]
        .as_str()
        .or_else(|| r["imported"][0]["id"].as_str())
        .unwrap_or_else(|| panic!("no score id in {r}"))
        .to_owned();
    run(&mut s, "score.open", json!({ "score": id }));
    (s, id)
}

fn strokes(s: &Session, id: &str, file_page: u32) -> usize {
    let sid: ScoreId = id.parse().unwrap();
    s.library().unwrap().scores[&sid]
        .annotations_on(PageKey(file_page))
        .len()
}

/// The first slot's destination rectangle in the viewport.
fn slot(s: &mut Session) -> [f64; 4] {
    let st = run(s, "view.state", json!({}));
    let d = &st["slots"][0]["dst"];
    [0, 1, 2, 3].map(|i| d[i].as_f64().unwrap())
}

fn page_line(y: f64) -> Value {
    json!({
        "space": "page",
        "page": 1,
        "points": [[100.0, y, 0.5], [300.0, y, 0.8], [500.0, y, 1.0]],
    })
}

#[test]
fn ink_1_and_2_draw_undo_redo() {
    let (mut s, id) = setup("draw", 2);
    let [x0, y0, x1, y1] = slot(&mut s);
    let (cy, w) = ((y0 + y1) / 2.0, x1 - x0);
    let r = run(
        &mut s,
        "annotate.stroke",
        json!({ "points": [[x0 + w * 0.2, cy, 0.4, 0], [x0 + w * 0.5, cy, 0.9, 16], [x0 + w * 0.8, cy, 1.0, 32]] }),
    );
    assert_eq!(r["drawn"], true);
    assert_eq!(r["page"], 1);
    assert_eq!(r["tool"], "pen");
    assert_eq!(strokes(&s, &id, 0), 1);
    let list = run(&mut s, "annotate.page.list", json!({}));
    let a = &list["annotations"][0];
    assert_eq!(a["type"], "stroke");
    // In page space: across the middle of a letter page, y up.
    let p0 = &a["points"][0];
    assert!(
        (p0["x"].as_f64().unwrap() - 612.0 * 0.2).abs() < 2.0,
        "{p0}"
    );
    assert!((p0["y"].as_f64().unwrap() - 396.0).abs() < 2.0, "{p0}");
    assert!((p0["pressure"].as_f64().unwrap() - 0.4).abs() < 1e-6);

    run(&mut s, "edit.undo", json!({ "score": id }));
    assert_eq!(strokes(&s, &id, 0), 0);
    run(&mut s, "edit.redo", json!({ "score": id }));
    assert_eq!(strokes(&s, &id, 0), 1);
}

#[test]
fn ink_3_rotated_page_maps_to_source_space() {
    let (mut s, id) = setup("rotate", 1);
    run(
        &mut s,
        "page.rotate",
        json!({ "score": id, "page": 1, "degrees": 90 }),
    );
    let [x0, y0, x1, y1] = slot(&mut s);
    run(
        &mut s,
        "annotate.stroke",
        json!({ "points": [[x0 + 1.0, y0 + 1.0], [x1 - 1.0, y1 - 1.0]] }),
    );
    let list = run(&mut s, "annotate.page.list", json!({}));
    let pts = &list["annotations"][0]["points"];
    let (sx, sy) = (pts[0]["x"].as_f64().unwrap(), pts[0]["y"].as_f64().unwrap());
    let (ex, ey) = (pts[1]["x"].as_f64().unwrap(), pts[1]["y"].as_f64().unwrap());
    // Turned clockwise, the screen's top left is the page's bottom left (0, 0).
    assert!(sx < 5.0 && sy < 5.0, "start {sx},{sy}");
    assert!(ex > 607.0 && ey > 787.0, "end {ex},{ey}");
    // Rotating back leaves the marking where it is in page space.
    let before = list["annotations"].clone();
    run(
        &mut s,
        "page.rotate",
        json!({ "score": id, "page": 1, "degrees": -90 }),
    );
    let after = run(&mut s, "annotate.page.list", json!({}));
    assert_eq!(after["annotations"], before);
}

#[test]
fn ink_8_stroke_eraser_removes_whole_strokes_in_one_step() {
    let (mut s, id) = setup("erase", 1);
    for y in [100.0, 200.0, 300.0] {
        run(&mut s, "annotate.stroke", page_line(y));
    }
    run(&mut s, "annotate.tool.strokeEraser", json!({ "width": 8 }));
    let r = run(
        &mut s,
        "annotate.stroke",
        json!({ "space": "page", "page": 1, "points": [[300.0, 150.0], [300.0, 350.0]] }),
    );
    assert_eq!(
        (r["erased"].as_u64(), r["added"].as_u64()),
        (Some(2), Some(0))
    );
    assert_eq!(strokes(&s, &id, 0), 1);
    run(&mut s, "edit.undo", json!({ "score": id }));
    assert_eq!(strokes(&s, &id, 0), 3);
    // A gesture that touches nothing records nothing.
    run(&mut s, "edit.redo", json!({ "score": id }));
    let r = run(
        &mut s,
        "annotate.stroke",
        json!({ "space": "page", "page": 1, "points": [[50.0, 700.0]] }),
    );
    assert_eq!(r["drawn"], false);
    run(&mut s, "edit.undo", json!({ "score": id }));
    assert_eq!(strokes(&s, &id, 0), 3, "the miss wasn't an undo step");
}

#[test]
fn ink_9_pixel_eraser_splits_and_undo_restores() {
    let (mut s, id) = setup("pixel", 1);
    run(&mut s, "annotate.stroke", page_line(400.0));
    let original = run(&mut s, "annotate.page.list", json!({}))["annotations"].clone();
    run(&mut s, "annotate.tool.pixelEraser", json!({}));
    let r = run(
        &mut s,
        "annotate.stroke",
        json!({ "space": "page", "page": 1, "points": [[300.0, 450.0], [300.0, 350.0]] }),
    );
    assert_eq!(
        (r["erased"].as_u64(), r["added"].as_u64()),
        (Some(1), Some(2))
    );
    let list = run(&mut s, "annotate.page.list", json!({}));
    let pieces = list["annotations"].as_array().unwrap();
    assert_eq!(pieces.len(), 2);
    for p in pieces {
        assert_eq!(p["color"], original[0]["color"]);
        assert_eq!(p["width"], original[0]["width"]);
    }
    run(&mut s, "edit.undo", json!({ "score": id }));
    let back = run(&mut s, "annotate.page.list", json!({}));
    assert_eq!(back["annotations"], original);
}

#[test]
fn ink_10_locked_or_hidden_layer_refuses_drawing() {
    let (mut s, id) = setup("locked", 1);
    let sid: ScoreId = id.parse().unwrap();
    let mut layer = s.library().unwrap().scores[&sid].layers[0].clone();
    layer.locked = true;
    s.library
        .as_mut()
        .unwrap()
        .commit(
            "Lock",
            Transaction::single(Op::SetLayer {
                score: sid,
                layer: layer.clone(),
            }),
        )
        .unwrap();
    assert_eq!(
        fails(&mut s, "annotate.stroke", page_line(100.0)),
        "invalid_edit"
    );
    assert_eq!(strokes(&s, &id, 0), 0);
    layer.locked = false;
    layer.visible = false;
    s.library
        .as_mut()
        .unwrap()
        .commit(
            "Hide",
            Transaction::single(Op::SetLayer { score: sid, layer }),
        )
        .unwrap();
    assert_eq!(
        fails(&mut s, "annotate.stroke", page_line(100.0)),
        "invalid_edit"
    );
}

#[test]
fn ink_12_markings_follow_their_page() {
    let (mut s, id) = setup("move", 3);
    run(
        &mut s,
        "annotate.stroke",
        json!({ "space": "page", "page": 2, "points": [[10.0, 10.0], [20.0, 20.0]] }),
    );
    assert_eq!(strokes(&s, &id, 1), 1);
    run(
        &mut s,
        "page.move",
        json!({ "score": id, "from": 2, "to": 3 }),
    );
    assert_eq!(strokes(&s, &id, 1), 1, "stored by source page");
    let at3 = run(&mut s, "annotate.page.list", json!({ "page": 3 }));
    assert_eq!(at3["filePage"], 2);
    assert_eq!(at3["annotations"].as_array().unwrap().len(), 1);
}

#[test]
fn ink_13_too_many_points_are_refused() {
    let (mut s, _) = setup("many", 1);
    let pts: Vec<Value> = (0..12_000)
        .map(|i| json!([f64::from(i) * 0.05, 100.0]))
        .collect();
    let code = fails(
        &mut s,
        "annotate.stroke",
        json!({ "space": "page", "page": 1, "points": pts }),
    );
    assert_eq!(code, "ink_too_many_points");
}

#[test]
fn ink_17_and_18_draw_with_touch() {
    let (mut s, id) = setup("touch", 1);
    let mut line = page_line(100.0);
    line["device"] = json!("mouse");
    let r = run(&mut s, "annotate.stroke", line.clone());
    assert_eq!(
        (r["drawn"].as_bool(), r["reason"].as_str()),
        (Some(false), Some("penOnly"))
    );
    line["device"] = json!("touch");
    assert_eq!(run(&mut s, "annotate.stroke", line.clone())["drawn"], false);
    assert_eq!(strokes(&s, &id, 0), 0);

    let r = run(&mut s, "input.pen.only", json!({}));
    assert_eq!(r["drawWithTouch"], true);
    line["device"] = json!("mouse");
    assert_eq!(run(&mut s, "annotate.stroke", line)["drawn"], true);
    let list = run(&mut s, "annotate.page.list", json!({}));
    assert!(
        list["annotations"][0]["points"]
            .as_array()
            .unwrap()
            .iter()
            .all(|p| p["pressure"] == 1.0),
        "mouse draws at full width"
    );
    run(&mut s, "input.pen.only", json!({ "on": true }));
    assert_eq!(run(&mut s, "annotate.state", json!({}))["penOnly"], true);
}

#[test]
fn ink_19_each_tool_keeps_its_own_colour() {
    let mut s = Session::new();
    run(&mut s, "annotate.tool.pen", json!({ "color": "#d32f2f" }));
    run(&mut s, "annotate.tool.highlighter", json!({}));
    let st = run(&mut s, "annotate.swatch.select", json!({ "index": 4 }));
    assert_eq!(st["highlighter"]["color"], st["highlighterSwatches"][3]);
    assert_eq!(st["pen"]["color"], "#d32f2fff");
    assert_eq!(st["tool"], "highlighter");
    // Width slider on the eraser.
    run(&mut s, "annotate.tool.strokeEraser", json!({}));
    let st = run(&mut s, "annotate.style.set", json!({ "width": 20 }));
    assert_eq!(st["eraser"]["width"], 20.0);
    // Whiteout, then a swatch returns the pen to ink.
    run(&mut s, "annotate.tool.whiteout", json!({}));
    assert_eq!(run(&mut s, "annotate.state", json!({}))["whiteoutOn"], true);
    let st = run(&mut s, "annotate.style.set", json!({ "color": "#000000" }));
    assert_eq!(st["whiteoutOn"], false);
}

#[test]
fn ink_shows_in_the_reader_frame() {
    let (mut s, _) = setup("frame", 1);
    run(
        &mut s,
        "annotate.tool.pen",
        json!({ "color": "#ff0000", "width": 12 }),
    );
    let before = s.reader_frame(true).unwrap()[0].bitmap.clone().unwrap();
    run(&mut s, "annotate.stroke", page_line(396.0));
    let frame = s.reader_frame(true).unwrap();
    let after = frame[0].bitmap.clone().unwrap();
    assert_eq!((before.width, before.height), (after.width, after.height));
    // The middle of the line, in the bitmap: x = 300 pt, y = 396 pt from the bottom.
    let sx = f64::from(after.width) / 612.0;
    let (x, y) = ((300.0 * sx) as u32, ((792.0 - 396.0) * sx) as u32);
    let px = after.pixel(x, y).unwrap();
    assert!(px[0] > 200 && px[1] < 60 && px[2] < 60, "{px:?}");
    // Cached: the same frame again is the same bitmap.
    let again = s.reader_frame(false).unwrap()[0].bitmap.clone().unwrap();
    assert!(std::sync::Arc::ptr_eq(&after, &again));
    // The stored stroke is a stroke with the chosen style.
    let sid = s.score.as_ref().unwrap().library_score.unwrap();
    let a = &s.library().unwrap().scores[&sid].annotations_on(PageKey(0))[0];
    let AnnotationKind::Stroke(st) = &a.kind else {
        panic!("not a stroke");
    };
    assert_eq!(st.width, 12.0);
}

#[test]
fn ink_16_bad_params_are_errors() {
    let (mut s, _) = setup("bad", 1);
    let cases = [
        json!({}),
        json!({ "points": 3 }),
        json!({ "points": [] }),
        json!({ "points": [[1]] }),
        json!({ "points": [[1, "a"]] }),
        json!({ "points": [[1, 2, 3, 4, 5]] }),
        json!({ "points": [[-1e9, -1e9]] }),
        json!({ "points": [[1, 2]], "space": "screen" }),
        json!({ "points": [[1, 2]], "device": "finger" }),
        json!({ "points": [[1, 2]], "space": "page" }),
        json!({ "points": [[1, 2]], "space": "page", "page": 9 }),
        json!({ "points": [[1, 2]], "page": 1 }),
        json!({ "points": [[1, 2]], "bogus": 1 }),
    ];
    for c in cases {
        assert!(s.dispatch("annotate.stroke", &c).is_err(), "{c}");
    }
    for (id, p) in [
        ("annotate.tool.pen", json!({ "width": 100 })),
        ("annotate.tool.pen", json!({ "color": "red" })),
        ("annotate.tool.whiteout", json!({ "color": "#ffffff" })),
        ("annotate.tool.pixelEraser", json!({ "width": 0 })),
        ("annotate.style.set", json!({})),
        ("input.pen.only", json!({ "on": "yes" })),
        ("annotate.page.list", json!({ "page": 0 })),
    ] {
        assert!(s.dispatch(id, &p).is_err(), "{id} {p}");
    }
}

#[test]
fn markup_needs_a_library_score() {
    let mut s = Session::new();
    assert_eq!(fails(&mut s, "annotate.stroke", page_line(1.0)), "disabled");
    let bytes = PdfBuilder::new().pages(1, PageSpec::default()).build();
    s.open_score_bytes(std::sync::Arc::new(bytes), None, "loose.pdf")
        .unwrap();
    assert_eq!(fails(&mut s, "annotate.stroke", page_line(1.0)), "disabled");
    assert_eq!(s.is_enabled("annotate.tool.pen"), Some(true));
}

#[test]
fn ink_24_touch_lock_ignores_touch_in_markup_mode() {
    let (mut s, id) = setup("lock", 1);
    run(&mut s, "input.pen.only", json!({ "on": false }));
    assert_eq!(
        fails(&mut s, "input.touch.lock", json!({ "on": true })),
        "disabled"
    );
    run(&mut s, "annotate.mode.set", json!({ "on": true }));
    assert_eq!(
        run(&mut s, "input.touch.lock", json!({}))["touchLock"],
        true
    );
    let mut line = page_line(100.0);
    line["device"] = json!("touch");
    let r = run(&mut s, "annotate.stroke", line.clone());
    assert_eq!(
        (r["drawn"].as_bool(), r["reason"].as_str()),
        (Some(false), Some("touchLock"))
    );
    line["device"] = json!("pen");
    assert_eq!(run(&mut s, "annotate.stroke", line.clone())["drawn"], true);
    line["device"] = json!("mouse");
    assert_eq!(
        run(&mut s, "annotate.stroke", line)["drawn"],
        true,
        "the mouse isn't touch"
    );
    assert_eq!(strokes(&s, &id, 0), 2);
    // The lock is kept but not applied outside markup mode.
    run(&mut s, "annotate.mode.set", json!({ "on": false }));
    assert_eq!(run(&mut s, "annotate.state", json!({}))["touchLock"], true);
}

#[test]
fn ink_25_and_26_palette_replaces_the_selected_swatch() {
    let mut s = Session::new();
    let st = run(&mut s, "annotate.swatch.select", json!({ "index": 2 }));
    assert_eq!(st["swatch"], 2);
    assert_eq!(st["pen"]["color"], st["swatches"][1]);
    let lavender = st["palette"][29].as_str().unwrap().to_owned();
    let st = run(&mut s, "annotate.swatch.set", json!({ "color": lavender }));
    assert_eq!(st["swatches"][1].as_str().unwrap(), lavender);
    assert_eq!(st["pen"]["color"].as_str().unwrap(), lavender);
    assert_eq!(st["swatch"], 2);
    assert_eq!(
        fails(&mut s, "annotate.swatch.set", json!({ "color": "#123456" })),
        "bad_params"
    );
    // INK-32: the highlighter has its own swatches and palette.
    let st = run(&mut s, "annotate.tool.highlighter", json!({}));
    assert_eq!(st["swatch"], 1);
    assert_ne!(st["swatches"], st["penSwatches"]);
    assert_eq!(st["swatches"], st["highlighterSwatches"]);
    assert_eq!(
        fails(&mut s, "annotate.swatch.set", json!({ "color": lavender })),
        "bad_params",
        "the pen palette's lavender isn't a highlighter colour"
    );
    let mint = st["highlighterPalette"][22].as_str().unwrap().to_owned();
    let st = run(
        &mut s,
        "annotate.swatch.set",
        json!({ "color": mint, "index": 3 }),
    );
    assert_eq!(st["highlighterSwatches"][2].as_str().unwrap(), mint);
    assert_eq!(st["highlighter"]["color"].as_str().unwrap(), mint);
    assert_eq!(
        st["penSwatches"][1].as_str().unwrap(),
        lavender,
        "pen swatches untouched"
    );
    run(&mut s, "annotate.tool.strokeEraser", json!({}));
    assert_eq!(
        fails(&mut s, "annotate.swatch.select", json!({ "index": 1 })),
        "bad_params"
    );
    for bad in [json!({ "index": 0 }), json!({ "index": 9 }), json!({})] {
        assert!(s.dispatch("annotate.swatch.select", &bad).is_err());
    }
}

#[test]
fn ink_27_ruler_straightens_strokes_that_start_at_its_edge() {
    let (mut s, _) = setup("ruler", 1);
    run(&mut s, "annotate.mode.set", json!({ "on": true }));
    let st = run(&mut s, "annotate.ruler.show", json!({}));
    // Across the whole reader, centred vertically (the default viewport is 1280 × 800).
    assert_eq!(st["ruler"]["y"], 400.0, "{st}");
    run(&mut s, "annotate.ruler.set", json!({ "y": 300.0 }));
    let edge = 300.0 + crate::markup::RULER_HALF_HEIGHT;
    let [x0, _, x1, _] = slot(&mut s);
    let wobble: Vec<Value> = (0..40)
        .map(|i| {
            let x = x0 + 5.0 + (x1 - x0 - 10.0) * f64::from(i) / 39.0;
            json!([x, edge + 5.0 + f64::from(i % 3)])
        })
        .collect();
    run(&mut s, "annotate.stroke", json!({ "points": wobble }));
    let list = run(&mut s, "annotate.page.list", json!({}));
    let pts = list["annotations"][0]["points"].as_array().unwrap();
    assert_eq!(pts.len(), 2, "ruled into a straight line");
    assert!((pts[0]["y"].as_f64().unwrap() - pts[1]["y"].as_f64().unwrap()).abs() < 1e-3);
    // Far from both edges: freehand.
    let free: Vec<Value> = (0..10)
        .map(|i| json!([x0 + 20.0 + f64::from(i) * 5.0, 600.0 + f64::from(i % 2)]))
        .collect();
    run(&mut s, "annotate.stroke", json!({ "points": free }));
    let list = run(&mut s, "annotate.page.list", json!({}));
    assert_eq!(
        list["annotations"][1]["points"].as_array().unwrap().len(),
        10
    );
    // Out of range moves are kept on screen; leaving markup mode removes the ruler.
    let st = run(&mut s, "annotate.ruler.set", json!({ "y": -50 }));
    assert_eq!(st["ruler"]["y"], 0.0);
    assert!(
        s.dispatch("annotate.ruler.set", &json!({ "x": 1, "y": 2 }))
            .is_err()
    );
    run(&mut s, "annotate.mode.set", json!({ "on": false }));
    assert!(run(&mut s, "annotate.state", json!({}))["ruler"].is_null());
}

#[test]
fn ink_29_and_30_highlighter_nib_follows_the_setting() {
    let (mut s, id) = setup("nib", 1);
    run(&mut s, "annotate.tool.highlighter", json!({}));
    run(&mut s, "annotate.stroke", page_line(100.0));
    run(
        &mut s,
        "annotate.highlighter.roundEnds",
        json!({ "on": true }),
    );
    run(&mut s, "annotate.stroke", page_line(200.0));
    run(&mut s, "annotate.tool.pen", json!({}));
    run(&mut s, "annotate.stroke", page_line(300.0));
    let sid: ScoreId = id.parse().unwrap();
    let nibs: Vec<scorecraft_doc::Nib> = s.library().unwrap().scores[&sid]
        .annotations_on(PageKey(0))
        .iter()
        .map(|a| match &a.kind {
            AnnotationKind::Stroke(st) => st.nib,
            _ => panic!("not a stroke"),
        })
        .collect();
    use scorecraft_doc::Nib::{Chisel, Round};
    assert_eq!(nibs, vec![Chisel, Round, Round]);
    assert_eq!(
        run(&mut s, "annotate.state", json!({}))["highlighterRoundEnds"],
        true
    );
}

#[test]
fn ink_28_white_pen_is_whiteout() {
    let (mut s, id) = setup("white", 1);
    let st = run(&mut s, "annotate.tool.pen", json!({ "color": "#ffffff" }));
    assert_eq!(st["whiteoutOn"], true);
    run(&mut s, "annotate.stroke", page_line(100.0));
    let sid: ScoreId = id.parse().unwrap();
    let a = &s.library().unwrap().scores[&sid].annotations_on(PageKey(0))[0];
    let AnnotationKind::Stroke(st) = &a.kind else {
        panic!("not a stroke");
    };
    assert_eq!(st.tool, scorecraft_doc::Tool::Whiteout);
    let st = run(&mut s, "annotate.swatch.select", json!({ "index": 1 }));
    assert_eq!(st["whiteoutOn"], false);
}
