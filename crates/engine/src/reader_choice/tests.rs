//! PT-14: a default layout and turn mode, and a score's own choice over it
//! (`docs/spec/page-turning.md`).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use serde_json::{Value, json};

use scorecraft_testkit::pdf::{PageSpec, PdfBuilder};

use crate::Session;

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    s.dispatch(id, &p)
        .unwrap_or_else(|e| panic!("{id} {p}: {e} ({})", e.code()))
}

fn layout(s: &mut Session) -> (String, String) {
    let st = s.reader_state().unwrap();
    (
        st["layout"].as_str().unwrap().to_owned(),
        st["mode"].as_str().unwrap().to_owned(),
    )
}

#[test]
fn pt_14_a_score_changed_in_its_own_view_keeps_its_layout() {
    let dir = std::env::temp_dir().join(format!("scorecraft-choice-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut s = Session::new();
    s.set_clock(Box::new(|| 1_793_404_800_000));
    s.create_memory_library().unwrap();
    let mut ids = Vec::new();
    for i in 0..2 {
        let path = dir.join(format!("{i}.pdf"));
        std::fs::write(
            &path,
            PdfBuilder::new().pages(4, PageSpec::default()).build(),
        )
        .unwrap();
        let r = run(
            &mut s,
            "library.import.files",
            json!({ "paths": [path.to_string_lossy()] }),
        );
        ids.push(r["imported"][0]["score"].as_str().unwrap().to_owned());
    }
    // The default from settings: two pages, half-page turns.
    let d = run(
        &mut s,
        "view.defaults.set",
        json!({ "layout": "twoPage", "mode": "halfPage", "split": 0.4 }),
    );
    assert_eq!(d["defaults"]["mode"], "halfPage");
    run(&mut s, "score.open", json!({ "score": ids[0] }));
    assert_eq!(layout(&mut s), ("twoPage".into(), "halfPage".into()));
    // Score A is changed in its own view: single pages, full turns.
    run(&mut s, "view.layout.single", json!({}));
    run(&mut s, "view.turn.halfPage", json!({ "on": false }));
    // Score B still follows the default; A keeps its own when it opens again.
    run(&mut s, "score.open", json!({ "score": ids[1] }));
    assert_eq!(layout(&mut s), ("twoPage".into(), "halfPage".into()));
    run(&mut s, "score.open", json!({ "score": ids[0] }));
    assert_eq!(layout(&mut s), ("single".into(), "full".into()));
    // A new default reaches B, not A.
    run(
        &mut s,
        "view.defaults.set",
        json!({ "mode": "alternating" }),
    );
    assert_eq!(layout(&mut s), ("single".into(), "full".into()));
    run(&mut s, "score.open", json!({ "score": ids[1] }));
    assert_eq!(layout(&mut s), ("twoPage".into(), "alternating".into()));
    // The choice is kept in the library's settings (so it syncs) and is not an undo step.
    let key = format!("reader.{}", ids[0]);
    assert!(s.library().unwrap().settings.contains_key(&key));
    let undo = run(&mut s, "library.info", json!({}))["canUndo"].clone();
    assert!(undo.as_str().unwrap().starts_with("Import"), "{undo}");
    // A change made without `remember` (a list carrying the layout over) is not kept.
    run(&mut s, "view.layout.single", json!({ "remember": false }));
    run(&mut s, "score.open", json!({ "score": ids[1] }));
    assert_eq!(layout(&mut s), ("twoPage".into(), "alternating".into()));
    // Back to the default for A.
    run(&mut s, "score.open", json!({ "score": ids[0] }));
    let st = run(&mut s, "view.override.clear", json!({}));
    assert_eq!(
        (st["layout"].as_str(), st["mode"].as_str()),
        (Some("twoPage"), Some("alternating"))
    );
    assert!(!s.library().unwrap().settings.contains_key(&key));
    for bad in [
        json!({ "layout": "three" }),
        json!({ "mode": "sideways" }),
        json!({ "split": 5 }),
    ] {
        assert_eq!(
            s.dispatch("view.defaults.set", &bad).unwrap_err().code(),
            "bad_params"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}
