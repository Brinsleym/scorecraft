//! Export with ink (EXP-2; `docs/spec/ink.md`, INK-47..49).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::sync::Arc;

use serde_json::{Value, json};

use scorecraft_testkit::pdf::{PageSpec, PdfBuilder};

use crate::Session;

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    s.dispatch(id, &p)
        .unwrap_or_else(|e| panic!("{id} {p}: {e} ({})", e.code()))
}

fn setup(name: &str) -> (Session, String, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("scorecraft-export-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("score.pdf");
    std::fs::write(
        &path,
        PdfBuilder::new().pages(3, PageSpec::default()).build(),
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
    (s, id, dir)
}

/// A thick red pen line across score page `page` at height `y`.
fn red_line(s: &mut Session, page: u32, y: f64) {
    run(
        s,
        "annotate.tool.pen",
        json!({ "color": "#d02020", "width": 10.0 }),
    );
    run(
        s,
        "annotate.stroke",
        json!({ "points": [[100, y], [300, y], [500, y]], "space": "page", "page": page }),
    );
}

/// The RGB of page point (`x`, `y`) on page `i` of `pdf`, rendered at 1×.
fn pixel(pdf: &[u8], i: usize, x: f64, y: f64) -> [u8; 3] {
    let mut doc = scorecraft_pdf::Document::open(Arc::new(pdf.to_vec())).unwrap();
    let g = doc.page(i).unwrap().clone();
    let bm = doc.render(i, 1.0).unwrap();
    let px = bm
        .pixel(
            (x - f64::from(g.crop[0])) as u32,
            (f64::from(g.crop[3]) - y) as u32,
        )
        .unwrap();
    [px[0], px[1], px[2]]
}

#[test]
fn ink_47_flattened_export_follows_the_score_and_leaves_the_file() {
    let (mut s, id, dir) = setup("flat");
    red_line(&mut s, 2, 400.0);
    // Hide the score's first page: the export starts at the inked one.
    run(&mut s, "page.setHidden", json!({ "score": id, "page": 1 }));
    let out = dir.join("out.pdf");
    let r = run(
        &mut s,
        "score.export.pdf",
        json!({ "path": out.to_string_lossy() }),
    );
    assert_eq!(
        (r["pages"].as_u64(), r["markings"].as_u64()),
        (Some(2), Some(1))
    );
    let pdf = std::fs::read(&out).unwrap();
    let ink = pixel(&pdf, 0, 300.0, 400.0);
    assert!(
        ink[0] > 150 && ink[1] < 90,
        "the stroke is in the page: {ink:?}"
    );
    // Away from the ink, and on the page without ink, the export looks like the source.
    let src = std::fs::read(dir.join("score.pdf")).unwrap();
    assert_eq!(pixel(&pdf, 0, 300.0, 600.0), pixel(&src, 1, 300.0, 600.0));
    assert_eq!(pixel(&pdf, 1, 300.0, 400.0), pixel(&src, 2, 300.0, 400.0));
    // The imported file is the same bytes as before.
    assert_eq!(
        std::fs::read(dir.join("score.pdf")).unwrap(),
        PdfBuilder::new().pages(3, PageSpec::default()).build()
    );
}

#[test]
fn ink_48_annotation_export_writes_ink_annotations() {
    let (mut s, _, dir) = setup("annots");
    red_line(&mut s, 1, 300.0);
    run(&mut s, "annotate.tool.text", json!({}));
    run(
        &mut s,
        "annotate.text.add",
        json!({ "text": "dolce", "x": 100.0, "y": 700.0, "space": "page", "page": 1 }),
    );
    let out = dir.join("annots.pdf");
    let r = run(
        &mut s,
        "score.export.pdf",
        json!({ "path": out.to_string_lossy(), "mode": "annotations" }),
    );
    assert_eq!(r["markings"], 2);
    let pdf = std::fs::read(&out).unwrap();
    // The ink is drawn by its appearance stream.
    let ink = pixel(&pdf, 0, 300.0, 300.0);
    assert!(ink[0] > 150 && ink[1] < 90, "{ink:?}");
    let text = String::from_utf8_lossy(&pdf);
    assert!(!text.is_empty());
}

#[test]
fn ink_49_hidden_layers_stay_out_unless_chosen() {
    let (mut s, _, dir) = setup("layers");
    let l = run(&mut s, "annotate.layer.new", json!({ "name": "Bowings" }));
    let layer = l["layer"].as_str().unwrap().to_owned();
    red_line(&mut s, 1, 300.0);
    run(
        &mut s,
        "annotate.layer.setVisible",
        json!({ "layer": layer, "visible": false }),
    );
    let out = dir.join("a.pdf");
    let r = run(
        &mut s,
        "score.export.pdf",
        json!({ "path": out.to_string_lossy() }),
    );
    assert_eq!(r["markings"], 0);
    let r = run(
        &mut s,
        "score.export.pdf",
        json!({ "path": out.to_string_lossy(), "layers": [layer] }),
    );
    assert_eq!(r["markings"], 1);
    for bad in [
        json!({ "path": out.to_string_lossy(), "mode": "jpeg" }),
        json!({ "path": out.to_string_lossy(), "layers": ["nope"] }),
        json!({ "path": dir.to_string_lossy() }),
        json!({}),
    ] {
        assert!(s.dispatch("score.export.pdf", &bad).is_err(), "{bad}");
    }
}

#[test]
fn snips_go_into_the_export() {
    let (mut s, _, dir) = setup("snip");
    run(
        &mut s,
        "page.snip.create",
        json!({ "x0": 50, "y0": 650, "x1": 560, "y1": 750, "space": "page", "page": 2,
                "toX": 50, "toY": 130, "toPage": 1 }),
    );
    let out = dir.join("snip.pdf");
    let r = run(
        &mut s,
        "score.export.pdf",
        json!({ "path": out.to_string_lossy() }),
    );
    assert_eq!(r["markings"], 1);
    let pdf = std::fs::read(&out).unwrap();
    // The snip's frame is drawn where it was placed.
    let frame = pixel(&pdf, 0, 300.0, 30.5);
    assert!(frame.iter().all(|c| *c < 240), "{frame:?}");
}
