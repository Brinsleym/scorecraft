//! Layer and preset commands (ANNO-10, ANNO-4; `docs/spec/ink.md`).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use serde_json::{Value, json};

use scorecraft_doc::{PageKey, ScoreId};
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
    let dir = std::env::temp_dir().join(format!("scorecraft-layers-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("score.pdf");
    std::fs::write(
        &path,
        PdfBuilder::new().pages(1, PageSpec::default()).build(),
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

fn line(y: f64) -> Value {
    json!({ "space": "page", "page": 1, "points": [[100.0, y], [300.0, y]] })
}

fn layer_of_last_marking(s: &Session, id: &str) -> String {
    let sid: ScoreId = id.parse().unwrap();
    let anns = s.library().unwrap().scores[&sid].annotations_on(PageKey(0));
    anns.last().unwrap().layer.to_string()
}

#[test]
fn new_layers_become_active_and_take_new_markings() {
    let (mut s, id) = setup("new");
    let l = run(&mut s, "annotate.layer.list", json!({}));
    assert_eq!(l["layers"].as_array().unwrap().len(), 1);
    assert_eq!(l["layers"][0]["name"], "Layer 1");
    let r = run(&mut s, "annotate.layer.new", json!({}));
    let second = r["layer"].as_str().unwrap().to_owned();
    assert_eq!(r["layers"][1]["name"], "Layer 2");
    assert_eq!(r["active"].as_str(), Some(second.as_str()));
    run(&mut s, "annotate.stroke", line(100.0));
    assert_eq!(layer_of_last_marking(&s, &id), second);
    // Back to the first layer.
    let first = r["layers"][0]["layer"].as_str().unwrap().to_owned();
    run(
        &mut s,
        "annotate.layer.setActive",
        json!({ "layer": first }),
    );
    run(&mut s, "annotate.stroke", line(200.0));
    assert_eq!(layer_of_last_marking(&s, &id), first);
    let named = run(
        &mut s,
        "annotate.layer.new",
        json!({ "name": "  Performance " }),
    );
    assert_eq!(named["layers"][2]["name"], "Performance");
}

#[test]
fn hide_lock_rename_move_and_undo() {
    let (mut s, id) = setup("flags");
    let r = run(&mut s, "annotate.layer.new", json!({ "name": "Bowings" }));
    let b = r["layer"].as_str().unwrap().to_owned();
    let r = run(&mut s, "annotate.layer.setVisible", json!({ "layer": b }));
    assert_eq!(r["layers"][1]["visible"], false, "no value toggles");
    let r = run(
        &mut s,
        "annotate.layer.setVisible",
        json!({ "layer": b, "visible": true }),
    );
    assert_eq!(r["layers"][1]["visible"], true);
    let r = run(
        &mut s,
        "annotate.layer.setLocked",
        json!({ "layer": b, "locked": true }),
    );
    assert_eq!(r["layers"][1]["locked"], true);
    assert_eq!(
        fails(&mut s, "annotate.stroke", line(100.0)),
        "invalid_edit"
    );
    run(&mut s, "edit.undo", json!({ "score": id }));
    let r = run(
        &mut s,
        "annotate.layer.rename",
        json!({ "layer": b, "name": "Fingerings" }),
    );
    assert_eq!(r["layers"][1]["name"], "Fingerings");
    let r = run(
        &mut s,
        "annotate.layer.move",
        json!({ "layer": b, "to": 1 }),
    );
    assert_eq!(r["layers"][0]["layer"].as_str(), Some(b.as_str()));
    run(&mut s, "edit.undo", json!({ "score": id }));
    let r = run(&mut s, "annotate.layer.list", json!({}));
    assert_eq!(r["layers"][1]["layer"].as_str(), Some(b.as_str()));
}

#[test]
fn deleting_a_layer_deletes_its_markings_in_one_step() {
    let (mut s, id) = setup("delete");
    let r = run(&mut s, "annotate.layer.new", json!({}));
    let b = r["layer"].as_str().unwrap().to_owned();
    run(&mut s, "annotate.stroke", line(100.0));
    run(&mut s, "annotate.stroke", line(200.0));
    let r = run(&mut s, "annotate.layer.delete", json!({ "layer": b }));
    assert_eq!(r["layers"].as_array().unwrap().len(), 1);
    assert_eq!(
        r["active"], r["layers"][0]["layer"],
        "falls back to the top layer"
    );
    let sid: ScoreId = id.parse().unwrap();
    assert!(
        s.library().unwrap().scores[&sid]
            .annotations_on(PageKey(0))
            .is_empty()
    );
    run(&mut s, "edit.undo", json!({ "score": id }));
    let r = run(&mut s, "annotate.layer.list", json!({}));
    assert_eq!(r["layers"][1]["markings"], 2);
    let only = r["layers"][0]["layer"].as_str().unwrap().to_owned();
    run(&mut s, "annotate.layer.delete", json!({ "layer": b }));
    assert_eq!(
        fails(&mut s, "annotate.layer.delete", json!({ "layer": only })),
        "invalid_edit"
    );
}

#[test]
fn bad_layer_params_are_errors() {
    let (mut s, _) = setup("bad");
    for (id, p) in [
        ("annotate.layer.new", json!({ "name": "" })),
        ("annotate.layer.new", json!({ "name": "x".repeat(65) })),
        (
            "annotate.layer.rename",
            json!({ "layer": "nope", "name": "a" }),
        ),
        ("annotate.layer.setVisible", json!({})),
        (
            "annotate.layer.move",
            json!({ "layer": "0190a8c0-0000-7000-8000-000000000000", "to": 1 }),
        ),
        ("annotate.layer.list", json!({ "x": 1 })),
    ] {
        assert!(s.dispatch(id, &p).is_err(), "{id} {p}");
    }
    let l = run(&mut s, "annotate.layer.list", json!({}));
    let only = l["layers"][0]["layer"].as_str().unwrap().to_owned();
    assert!(
        s.dispatch("annotate.layer.move", &json!({ "layer": only, "to": 2 }))
            .is_err()
    );
    let mut fresh = Session::new();
    assert_eq!(
        fails(&mut fresh, "annotate.layer.list", json!({})),
        "disabled"
    );
}

#[test]
fn presets_save_and_restore_tool_colour_and_width() {
    let mut s = Session::new();
    run(&mut s, "annotate.tool.highlighter", json!({ "width": 20 }));
    run(&mut s, "annotate.swatch.select", json!({ "index": 2 }));
    let st = run(&mut s, "annotate.pen.preset.set", json!({ "name": "Pink" }));
    assert_eq!(st["presets"][0]["tool"], "highlighter");
    let pink = st["highlighter"]["color"].clone();
    run(
        &mut s,
        "annotate.tool.pen",
        json!({ "color": "#000000", "width": 1 }),
    );
    run(&mut s, "annotate.pen.preset.set", json!({ "name": "Fine" }));
    run(&mut s, "annotate.tool.highlighter", json!({ "width": 8 }));
    run(&mut s, "annotate.swatch.select", json!({ "index": 1 }));
    let st = run(
        &mut s,
        "annotate.pen.preset.apply",
        json!({ "name": "Pink" }),
    );
    assert_eq!(st["tool"], "highlighter");
    assert_eq!(st["highlighter"]["color"], pink);
    assert_eq!(st["highlighter"]["width"], 20.0);
    let st = run(
        &mut s,
        "annotate.pen.preset.apply",
        json!({ "name": "Fine" }),
    );
    assert_eq!(
        (st["tool"].as_str(), st["pen"]["width"].as_f64()),
        (Some("pen"), Some(1.0))
    );
    // Same name replaces; delete removes; unknown names are errors.
    run(&mut s, "annotate.pen.preset.set", json!({ "name": "Fine" }));
    let st = run(
        &mut s,
        "annotate.pen.preset.delete",
        json!({ "name": "Fine" }),
    );
    assert_eq!(st["presets"].as_array().unwrap().len(), 1);
    assert_eq!(
        fails(
            &mut s,
            "annotate.pen.preset.apply",
            json!({ "name": "Fine" })
        ),
        "not_found"
    );
    run(&mut s, "annotate.tool.strokeEraser", json!({}));
    assert_eq!(
        fails(&mut s, "annotate.pen.preset.set", json!({ "name": "E" })),
        "bad_params"
    );
    assert!(
        s.dispatch("annotate.pen.preset.set", &json!({ "name": " " }))
            .is_err()
    );
}
