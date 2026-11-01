//! Rectangle select (ANNO-12), shapes (ANNO-16) and the pen button (ANNO-11);
//! `docs/spec/ink.md`.
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use serde_json::{Value, json};

use scorecraft_doc::{AnnotationKind, Color, PageKey, ScoreId, Stroke};
use scorecraft_testkit::pdf::{PageSpec, PdfBuilder};

use crate::Session;

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    s.dispatch(id, &p)
        .unwrap_or_else(|e| panic!("{id} {p}: {e} ({})", e.code()))
}

fn fails(s: &mut Session, id: &str, p: Value) -> String {
    match s.dispatch(id, &p) {
        Ok(v) => panic!("{id} {p} should fail, got {v}"),
        Err(e) => e.code().to_owned(),
    }
}

fn setup(name: &str) -> (Session, String) {
    let dir = std::env::temp_dir().join(format!("scorecraft-select-{name}-{}", std::process::id()));
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

fn on(s: &Session, id: &str, page: u32) -> Vec<(String, AnnotationKind)> {
    let sid: ScoreId = id.parse().unwrap();
    s.library().unwrap().scores[&sid]
        .annotations_on(PageKey(page))
        .iter()
        .map(|a| (a.id.to_string(), a.kind.clone()))
        .collect()
}

fn stroke(kind: &AnnotationKind) -> &Stroke {
    match kind {
        AnnotationKind::Stroke(s) => s,
        other => panic!("not a stroke: {other:?}"),
    }
}

/// A horizontal page-space line on page `page` from `x0` to `x1` at `y`.
fn line(page: u32, x0: f64, x1: f64, y: f64) -> Value {
    json!({ "points": [[x0, y], [(x0 + x1) / 2.0, y], [x1, y]], "space": "page", "page": page })
}

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Value {
    json!({ "x0": x0, "y0": y0, "x1": x1, "y1": y1, "space": "page", "page": 1 })
}

fn first_x(kind: &AnnotationKind) -> f32 {
    stroke(kind).points[0].x
}

#[test]
fn ink_39_moving_a_selection_to_another_page_is_one_undo_step() {
    let (mut s, id) = setup("move");
    run(&mut s, "annotate.stroke", line(1, 100.0, 200.0, 500.0));
    run(&mut s, "annotate.stroke", line(1, 100.0, 200.0, 520.0));
    run(&mut s, "annotate.stroke", line(1, 100.0, 200.0, 100.0));
    run(&mut s, "annotate.tool.text", json!({}));
    run(
        &mut s,
        "annotate.text.add",
        json!({ "text": "rit.", "x": 120.0, "y": 560.0, "space": "page", "page": 1 }),
    );
    run(&mut s, "annotate.tool.pen", json!({}));
    // A stroke only partly inside: two of its three samples are outside.
    run(&mut s, "annotate.stroke", line(1, 180.0, 400.0, 540.0));
    let r = run(
        &mut s,
        "annotate.select.rect",
        rect(90.0, 480.0, 210.0, 580.0),
    );
    assert_eq!(r["count"], 3, "{r}");
    assert_eq!(r["actions"][0], "move");
    let state = run(&mut s, "annotate.state", json!({}));
    assert_eq!(state["tool"], "select");
    assert_eq!(
        state["selection"]["annotations"].as_array().unwrap().len(),
        3
    );
    let ids: Vec<String> = r["annotations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect();
    // The rectangle's centre is (150, 530); drop it at (350, 330) on page 2.
    let r = run(
        &mut s,
        "annotate.select.move",
        json!({ "x": 350.0, "y": 330.0, "space": "page", "page": 2 }),
    );
    assert_eq!(r["filePage"], 2);
    assert_eq!(
        on(&s, &id, 0).len(),
        2,
        "the far and the partial strokes stay"
    );
    let moved = on(&s, &id, 1);
    assert_eq!(moved.len(), 3);
    for (mid, _) in &moved {
        assert!(ids.contains(mid), "moving keeps ids");
    }
    let strokes: Vec<f32> = moved
        .iter()
        .filter(|(_, k)| matches!(k, AnnotationKind::Stroke(_)))
        .map(|(_, k)| first_x(k))
        .collect();
    assert_eq!(strokes, vec![300.0, 300.0]);
    run(&mut s, "edit.undo", json!({ "score": id }));
    assert_eq!(on(&s, &id, 0).len(), 5);
    assert!(on(&s, &id, 1).is_empty());
    // The selection now names markings on page 2 that undo took away.
    assert_eq!(
        fails(&mut s, "annotate.select.delete", json!({})),
        "bad_params"
    );
}

#[test]
fn ink_40_copy_scale_recolour_and_delete() {
    let (mut s, id) = setup("copy");
    run(&mut s, "annotate.stroke", line(1, 100.0, 200.0, 500.0));
    run(&mut s, "annotate.tool.highlighter", json!({}));
    run(&mut s, "annotate.stroke", line(1, 100.0, 200.0, 520.0));
    run(
        &mut s,
        "annotate.select.rect",
        rect(90.0, 480.0, 210.0, 540.0),
    );
    // Copy within the page: originals stay, copies get new ids.
    let r = run(
        &mut s,
        "annotate.select.copy",
        json!({ "x": 150.0, "y": 300.0, "space": "page", "page": 1 }),
    );
    let all = on(&s, &id, 0);
    assert_eq!(all.len(), 4);
    let copies: Vec<String> = r["annotations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect();
    assert!(!copies.contains(&all[0].0) && !copies.contains(&all[1].0));
    assert_eq!(stroke(&all[2].1).points[0].y, 290.0);
    // The copies are selected now: scale them 2x about their centre (150, 300).
    let pen_width = stroke(&all[2].1).width;
    run(&mut s, "annotate.select.scale", json!({ "factor": 2.0 }));
    let all = on(&s, &id, 0);
    let pen = stroke(&all[2].1);
    assert_eq!((pen.points[0].x, pen.points[0].y), (50.0, 280.0));
    assert!((pen.width - pen_width * 2.0).abs() < 1e-4);
    assert_eq!(
        fails(&mut s, "annotate.select.scale", json!({ "factor": 11.0 })),
        "bad_params"
    );
    // Recolour: both take the colour.
    run(
        &mut s,
        "annotate.select.recolor",
        json!({ "color": "#c03030" }),
    );
    let all = on(&s, &id, 0);
    assert_eq!(stroke(&all[2].1).color, Color::rgb(0xc0, 0x30, 0x30));
    let hl = stroke(&all[3].1).color;
    assert_eq!((hl.r, hl.g, hl.b), (0xc0, 0x30, 0x30));
    // Delete: both copies go in one step; undo brings both back.
    let r = run(&mut s, "annotate.select.delete", json!({}));
    assert_eq!(r["deleted"], 2);
    assert_eq!(on(&s, &id, 0).len(), 2);
    run(&mut s, "edit.undo", json!({ "score": id }));
    assert_eq!(on(&s, &id, 0).len(), 4);
}

#[test]
fn locked_layers_are_never_selected_and_empty_selections_offer_nothing() {
    let (mut s, id) = setup("locked");
    run(&mut s, "annotate.stroke", line(1, 100.0, 200.0, 500.0));
    let layers = run(&mut s, "annotate.layer.list", json!({}));
    let first = layers["layers"][0]["layer"].as_str().unwrap().to_owned();
    run(
        &mut s,
        "annotate.layer.setLocked",
        json!({ "layer": first, "locked": true }),
    );
    let r = run(&mut s, "annotate.select.rect", rect(0.0, 0.0, 612.0, 792.0));
    assert_eq!(r["count"], 0);
    assert!(r["actions"].as_array().unwrap().is_empty());
    assert_eq!(
        fails(
            &mut s,
            "annotate.select.move",
            json!({ "x": 1.0, "y": 1.0, "space": "page", "page": 1 })
        ),
        "bad_params"
    );
    run(&mut s, "annotate.select.clear", json!({}));
    assert!(run(&mut s, "annotate.state", json!({}))["selection"].is_null());
    let _ = id;
}

#[test]
fn ink_41_shapes_straighten_lines_and_ellipses() {
    let (mut s, id) = setup("shapes");
    let wobbly: Vec<Value> = (0..=40)
        .map(|k| {
            let x = 100.0 + f64::from(k) * 5.0;
            json!([x, 400.0 + (f64::from(k) * 0.9).sin() * 2.5])
        })
        .collect();
    let freehand = run(
        &mut s,
        "annotate.stroke",
        json!({ "points": wobbly, "space": "page", "page": 1 }),
    );
    assert!(freehand["shape"].is_null());
    let st = run(&mut s, "annotate.shapes", json!({}));
    assert_eq!(st["shapes"], true);
    let r = run(
        &mut s,
        "annotate.stroke",
        json!({ "points": wobbly, "space": "page", "page": 1 }),
    );
    assert_eq!(r["shape"], "line");
    let all = on(&s, &id, 0);
    let straight = stroke(&all[1].1);
    let y0 = straight.points[0].y;
    let y1 = straight.points.last().unwrap().y;
    for (k, p) in straight.points.iter().enumerate() {
        let t = k as f32 / (straight.points.len() - 1) as f32;
        assert!((p.y - (y0 + (y1 - y0) * t)).abs() < 1e-3);
    }
    let circle: Vec<Value> = (0..=60)
        .map(|k| {
            let a = f64::from(k) / 60.0 * std::f64::consts::TAU;
            json!([300.0 + 50.0 * a.cos(), 300.0 + 50.0 * a.sin()])
        })
        .collect();
    let r = run(
        &mut s,
        "annotate.stroke",
        json!({ "points": circle, "space": "page", "page": 1 }),
    );
    assert_eq!(r["shape"], "ellipse");
}

#[test]
fn ink_42_the_pen_button_switches_pen_and_eraser() {
    let (mut s, _) = setup("button");
    assert_eq!(
        run(&mut s, "annotate.state", json!({}))["penButton"],
        "eraser"
    );
    assert_eq!(
        run(&mut s, "annotate.pen.button", json!({}))["tool"],
        "eraser"
    );
    assert_eq!(run(&mut s, "annotate.pen.button", json!({}))["tool"], "pen");
    run(&mut s, "annotate.tool.highlighter", json!({}));
    assert_eq!(
        run(&mut s, "annotate.pen.button", json!({}))["tool"],
        "eraser"
    );
    assert_eq!(
        run(&mut s, "annotate.pen.button", json!({}))["tool"],
        "highlighter",
        "back to the tool before"
    );
    run(
        &mut s,
        "annotate.pen.buttonTool",
        json!({ "tool": "select" }),
    );
    assert_eq!(
        run(&mut s, "annotate.pen.button", json!({}))["tool"],
        "select"
    );
    assert_eq!(
        fails(
            &mut s,
            "annotate.pen.buttonTool",
            json!({ "tool": "lasso" })
        ),
        "bad_params"
    );
}
