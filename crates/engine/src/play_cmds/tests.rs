//! Playing a setlist (`docs/spec/performance.md`, PERF-1..4).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use serde_json::{Value, json};

use scorecraft_testkit::pdf::{PageSpec, PdfBuilder};

use crate::Session;

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    s.dispatch(id, &p)
        .unwrap_or_else(|e| panic!("{id} {p}: {e} ({})", e.code()))
}

/// A library with scores of `pages` pages each, titled A, B, C, …
fn library(name: &str, pages: &[usize]) -> (Session, Vec<String>) {
    let dir = std::env::temp_dir().join(format!("scorecraft-play-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut s = Session::new();
    s.set_clock(Box::new(|| 1_793_404_800_000));
    s.create_memory_library().unwrap();
    let mut ids = Vec::new();
    for (i, n) in pages.iter().enumerate() {
        let path = dir.join(format!("{i}.pdf"));
        let title = ((b'A' + i as u8) as char).to_string();
        std::fs::write(
            &path,
            PdfBuilder::new()
                .pages(*n, PageSpec::default())
                .title(&title)
                .build(),
        )
        .unwrap();
        let r = run(
            &mut s,
            "library.import.files",
            json!({ "paths": [path.to_string_lossy()] }),
        );
        ids.push(r["imported"][0]["score"].as_str().unwrap().to_owned());
    }
    (s, ids)
}

fn page(s: &mut Session) -> (u64, u64) {
    let st = s.reader_state().unwrap();
    (
        st["page"].as_u64().unwrap(),
        st["pageCount"].as_u64().unwrap(),
    )
}

#[test]
fn perf_1_turning_past_the_end_opens_the_next_entry_and_back() {
    let (mut s, ids) = library("flow", &[2, 3]);
    let list = run(&mut s, "list.new", json!({ "name": "Concert" }))["list"]
        .as_str()
        .unwrap()
        .to_owned();
    run(
        &mut s,
        "list.entry.add",
        json!({ "list": list, "score": ids[0] }),
    );
    run(
        &mut s,
        "list.entry.addPlaceholder",
        json!({ "list": list, "label": "Interval" }),
    );
    run(
        &mut s,
        "list.entry.add",
        json!({ "list": list, "score": ids[1] }),
    );
    let r = run(&mut s, "list.play", json!({ "list": list }));
    assert_eq!(
        (r["entry"].as_u64(), r["title"].as_str()),
        (Some(1), Some("A"))
    );
    assert_eq!(r["next"], "B");
    run(
        &mut s,
        "view.viewport.set",
        json!({ "width": 1000, "height": 700, "pixelsPerPoint": 2 }),
    );
    run(&mut s, "view.layout.single", json!({}));
    run(&mut s, "view.turn.next", json!({}));
    assert_eq!(page(&mut s), (2, 2));
    run(&mut s, "view.layout.twoPage", json!({}));
    // A pedal press on the last page goes on to the next score, past the interval.
    let r = run(&mut s, "input.key", json!({ "key": "PageDown" }));
    let st = run(&mut s, "list.state", json!({}));
    assert_eq!(
        (st["entry"].as_u64(), st["title"].as_str()),
        (Some(3), Some("B")),
        "{r}"
    );
    assert_eq!(page(&mut s), (1, 3));
    let v = s.reader_state().unwrap();
    assert_eq!(
        (v["layout"].as_str(), v["viewport"]["width"].as_f64()),
        (Some("twoPage"), Some(1000.0)),
        "the next score keeps the window and layout"
    );
    run(&mut s, "view.layout.single", json!({}));
    // Back from its first page: the previous score at its last page.
    let r = run(&mut s, "view.turn.previous", json!({}));
    assert_eq!(r["breaks"], json!(["Interval"]));
    assert_eq!(run(&mut s, "list.state", json!({}))["title"], "A");
    assert_eq!(page(&mut s), (2, 2));
    // Past the very end, nothing more turns.
    run(&mut s, "list.play", json!({ "list": list, "entry": 3 }));
    run(&mut s, "view.page.goto", json!({ "to": "last" }));
    assert_eq!(run(&mut s, "view.turn.next", json!({}))["turned"], false);
}

#[test]
fn perf_2_an_entry_plays_only_its_page_range() {
    let (mut s, ids) = library("range", &[5]);
    let list = run(&mut s, "list.new", json!({ "name": "Audition" }))["list"]
        .as_str()
        .unwrap()
        .to_owned();
    let e = run(
        &mut s,
        "list.entry.add",
        json!({ "list": list, "score": ids[0] }),
    );
    let entry = e["entry"].as_str().map(str::to_owned).unwrap_or_else(|| {
        run(&mut s, "list.get", json!({ "list": list }))["entries"][0]["id"]
            .as_str()
            .unwrap()
            .to_owned()
    });
    run(
        &mut s,
        "list.entry.pageRange.set",
        json!({ "list": list, "entry": entry, "first": 2, "last": 3 }),
    );
    run(&mut s, "list.play", json!({ "list": list }));
    assert_eq!(page(&mut s).1, 2, "pages 2 and 3 only");
    // Stopping shows the whole score again.
    run(&mut s, "list.stop", json!({}));
    assert_eq!(page(&mut s).1, 5);
    assert_eq!(run(&mut s, "list.state", json!({}))["playing"], false);
}

#[test]
fn perf_3_one_score_in_many_lists_is_one_score() {
    let (mut s, ids) = library("shared", &[1]);
    let mut lists = Vec::new();
    for name in ["Monday", "Friday"] {
        let l = run(&mut s, "list.new", json!({ "name": name }))["list"]
            .as_str()
            .unwrap()
            .to_owned();
        run(
            &mut s,
            "list.entry.add",
            json!({ "list": l, "score": ids[0] }),
        );
        run(
            &mut s,
            "list.entry.add",
            json!({ "list": l, "score": ids[0] }),
        );
        lists.push(l);
    }
    run(
        &mut s,
        "score.metadata.set",
        json!({ "score": ids[0], "title": "Renamed" }),
    );
    assert_eq!(
        run(&mut s, "library.scores", json!({}))["scores"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    for l in &lists {
        run(&mut s, "list.play", json!({ "list": l }));
        assert_eq!(run(&mut s, "list.state", json!({}))["title"], "Renamed");
    }
    run(&mut s, "list.delete", json!({ "list": lists[0] }));
    assert_eq!(
        run(&mut s, "library.scores", json!({}))["scores"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn perf_4_bad_plays_are_refused_and_opening_by_hand_stops() {
    let (mut s, ids) = library("bad", &[1]);
    let list = run(&mut s, "list.new", json!({ "name": "Empty" }))["list"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(s.dispatch("list.play", &json!({ "list": list })).is_err());
    run(
        &mut s,
        "list.entry.add",
        json!({ "list": list, "score": ids[0] }),
    );
    assert!(
        s.dispatch("list.play", &json!({ "list": list, "entry": 5 }))
            .is_err()
    );
    run(&mut s, "list.play", json!({ "list": list }));
    run(&mut s, "score.open", json!({ "score": ids[0] }));
    assert_eq!(run(&mut s, "list.state", json!({}))["playing"], false);
}
