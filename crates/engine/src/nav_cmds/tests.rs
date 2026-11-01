//! Score Shortcuts and Snips (`docs/spec/performance.md`, PERF-9..11).
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
    let dir = std::env::temp_dir().join(format!("scorecraft-nav-{name}-{}", std::process::id()));
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
    (s, id)
}

fn kinds(s: &Session, id: &str, page: u32) -> Vec<AnnotationKind> {
    let sid: ScoreId = id.parse().unwrap();
    s.library().unwrap().scores[&sid]
        .annotations_on(PageKey(page))
        .iter()
        .map(|a| a.kind.clone())
        .collect()
}

#[test]
fn perf_9_a_shortcut_jumps_to_its_page_when_tapped() {
    let (mut s, id) = setup("shortcut");
    let r = run(
        &mut s,
        "nav.shortcut.add",
        json!({ "x0": 500, "y0": 700, "x1": 560, "y1": 740, "space": "page", "page": 1, "target": 3 }),
    );
    let aid = r["annotation"].as_str().unwrap().to_owned();
    let miss = run(
        &mut s,
        "nav.shortcut.follow",
        json!({ "x": 100, "y": 100, "space": "page", "page": 1 }),
    );
    assert_eq!(miss["followed"], false);
    // The reader can outline it where it shows, and a tap there follows it.
    let rects = s.shortcut_rects();
    assert_eq!(rects.len(), 1);
    let ([x0, y0, x1, y1], target) = rects[0];
    assert_eq!(target, Some(3));
    assert!(x1 > x0 && y1 > y0);
    let tap = run(
        &mut s,
        "nav.shortcut.follow",
        json!({ "x": (x0 + x1) / 2.0, "y": (y0 + y1) / 2.0 }),
    );
    assert_eq!(
        tap["followed"], true,
        "a tap in view space on the outline follows it"
    );
    run(&mut s, "view.page.goto", json!({ "page": 1 }));
    let hit = run(
        &mut s,
        "nav.shortcut.follow",
        json!({ "x": 530, "y": 720, "space": "page", "page": 1 }),
    );
    assert_eq!(hit["followed"], true);
    assert_eq!(s.reader_state().unwrap()["page"], 3);
    // A hidden target can't be jumped to.
    run(&mut s, "page.setHidden", json!({ "score": id, "page": 3 }));
    let hidden = run(
        &mut s,
        "nav.shortcut.follow",
        json!({ "x": 530, "y": 720, "space": "page", "page": 1 }),
    );
    assert_eq!(hidden["followed"], false);
    run(&mut s, "annotate.delete", json!({ "annotation": aid }));
    assert!(kinds(&s, &id, 0).is_empty());
    let bad = s.dispatch(
        "nav.shortcut.add",
        &json!({ "x0": 1, "y0": 1, "x1": 50, "y1": 50, "space": "page", "page": 1, "target": 9 }),
    );
    assert!(bad.is_err());
}

#[test]
fn perf_10_a_snip_shows_another_pages_bars_and_moves() {
    let (mut s, id) = setup("snip");
    // The top of page 2 shown at the foot of page 1.
    let r = run(
        &mut s,
        "page.snip.create",
        json!({ "x0": 50, "y0": 650, "x1": 560, "y1": 750, "space": "page", "page": 2,
                "toX": 50, "toY": 130, "toPage": 1 }),
    );
    assert_eq!(
        (r["from"].as_u64(), r["filePage"].as_u64()),
        (Some(2), Some(1))
    );
    let k = kinds(&s, &id, 0);
    let AnnotationKind::Snip(snip) = &k[0] else {
        panic!("{k:?}")
    };
    assert_eq!(snip.from.page, PageKey(1));
    assert_eq!(
        (snip.to.rect.width(), snip.to.rect.height()),
        (510.0, 100.0)
    );
    assert_eq!(snip.to.rect.y1, 130.0);
    // Moved below the page's foot, it is kept on the page.
    let aid = r["annotation"].as_str().unwrap();
    let r = run(
        &mut s,
        "page.snip.place",
        json!({ "annotation": aid, "x": 50, "y": 40, "space": "page", "page": 1 }),
    );
    assert_eq!(r["rect"][1], 0.0);
    // Resized to half its width, it keeps its shape.
    let r = run(
        &mut s,
        "page.snip.place",
        json!({ "annotation": aid, "x": 50, "y": 300, "space": "page", "page": 1, "width": 255 }),
    );
    let (w, h) = (
        r["rect"][2].as_f64().unwrap() - r["rect"][0].as_f64().unwrap(),
        r["rect"][3].as_f64().unwrap() - r["rect"][1].as_f64().unwrap(),
    );
    assert_eq!((w, h), (255.0, 50.0));
    assert!(
        s.dispatch(
            "page.snip.place",
            &json!({ "annotation": aid, "x": 1, "y": 1, "space": "page", "page": 1, "width": 5000 })
        )
        .is_err()
    );
    run(&mut s, "edit.undo", json!({ "score": id }));
    run(&mut s, "edit.undo", json!({ "score": id }));
    let AnnotationKind::Snip(snip) = &kinds(&s, &id, 0)[0] else {
        panic!()
    };
    assert_eq!(snip.to.rect.y1, 130.0, "undo puts it back");
}

#[test]
fn perf_11_a_snip_is_drawn_into_its_page() {
    let (mut s, _) = setup("draw");
    let dir = std::env::temp_dir().join(format!("scorecraft-nav-draw-out-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (a, b) = (dir.join("before.png"), dir.join("after.png"));
    run(
        &mut s,
        "view.render",
        json!({ "page": 1, "dpi": 36, "out": a.to_string_lossy() }),
    );
    run(
        &mut s,
        "page.snip.create",
        json!({ "x0": 0, "y0": 0, "x1": 612, "y1": 792, "space": "page", "page": 2,
                "toX": 0, "toY": 792, "toPage": 1 }),
    );
    run(
        &mut s,
        "view.render",
        json!({ "page": 1, "dpi": 36, "out": b.to_string_lossy() }),
    );
    assert_ne!(
        std::fs::read(&a).unwrap(),
        std::fs::read(&b).unwrap(),
        "page 1 now shows the snip"
    );
}
