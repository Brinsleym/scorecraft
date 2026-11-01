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
    // A pedal press on the last page goes on to the interval's own page and stops there.
    run(&mut s, "input.key", json!({ "key": "PageDown" }));
    let st = run(&mut s, "list.state", json!({}));
    assert_eq!(
        (st["entry"].as_u64(), st["title"].as_str()),
        (Some(2), Some("Interval"))
    );
    assert_eq!(page(&mut s), (1, 1));
    // The next turn goes on to the next score.
    let r = run(&mut s, "view.turn.next", json!({}));
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
    // Back from its first page: the interval, then the previous score at its last page.
    run(&mut s, "view.turn.previous", json!({}));
    assert_eq!(run(&mut s, "list.state", json!({}))["title"], "Interval");
    let r = run(&mut s, "view.turn.previous", json!({}));
    assert_eq!(r["breaks"], json!([]));
    assert_eq!(run(&mut s, "list.state", json!({}))["title"], "A");
    // A was switched to two pages in its own view, so it keeps them (PT-14): its last spread.
    assert_eq!(s.reader_state().unwrap()["layout"], "twoPage");
    assert_eq!(page(&mut s), (1, 2));
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
    // TURN-7: list turns need a playing list.
    assert!(s.dispatch("list.turn.next", &json!({})).is_err());
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
    assert_eq!(
        run(&mut s, "list.turn.next", json!({}))["turned"],
        false,
        "one page, one entry"
    );
    run(&mut s, "score.open", json!({ "score": ids[0] }));
    assert_eq!(run(&mut s, "list.state", json!({}))["playing"], false);
}

#[test]
fn perf_14_a_placeholder_is_an_a4_page_with_its_name_that_takes_ink() {
    let (mut s, ids) = library("placeholder", &[2]);
    let list = run(&mut s, "list.new", json!({ "name": "Gala" }))["list"]
        .as_str()
        .unwrap()
        .to_owned();
    // Control characters, bidi overrides and runs of spaces are cleaned out of the name.
    let r = run(
        &mut s,
        "list.entry.addPlaceholder",
        json!({ "list": list, "label": "  Inter\u{202e}val\n\t talk  " }),
    );
    let page_id: scorecraft_doc::ScoreId = r["page"].as_str().unwrap().parse().unwrap();
    let lib = s.library().unwrap();
    let page = &lib.scores[&page_id];
    assert_eq!(page.metadata.title, "Interval talk");
    assert_eq!(page.pages.len(), 1);
    let note = match &page.annotations_on(scorecraft_doc::PageKey(0))[0].kind {
        scorecraft_doc::AnnotationKind::Text(t) => t.clone(),
        other => panic!("{other:?}"),
    };
    assert_eq!(note.text, "Interval talk");
    assert!(note.style.size >= 36.0);
    let c = note.rect.center();
    assert!(
        (c.x - 595.28 / 2.0).abs() < 0.5 && (c.y - 841.89 / 2.0).abs() < 0.5,
        "{c:?}"
    );
    // The page is the list's, not the library's: it is not listed, searched or recent.
    let listed = run(&mut s, "library.scores", json!({}));
    assert_eq!(listed["scores"].as_array().unwrap().len(), 1, "{listed}");
    let found = run(&mut s, "library.search", json!({ "query": "interval" }));
    assert_eq!(found["count"], 0, "{found}");
    // Playing stops on it, and it takes ink and text like any score.
    run(&mut s, "list.play", json!({ "list": list }));
    assert_eq!(
        run(&mut s, "list.state", json!({}))["title"],
        "Interval talk"
    );
    run(
        &mut s,
        "annotate.stroke",
        json!({ "points": [[100, 100], [200, 120]], "space": "page", "page": 1 }),
    );
    assert_eq!(
        s.library().unwrap().scores[&page_id]
            .annotations_on(scorecraft_doc::PageKey(0))
            .len(),
        2
    );
    // Removing the entry sends the page to Recently Deleted with its ink; undo brings both back.
    let entry = run(&mut s, "list.get", json!({ "list": list }))["entries"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    run(
        &mut s,
        "list.entry.remove",
        json!({ "list": list, "entry": entry }),
    );
    assert!(s.library().unwrap().is_trashed(page_id));
    run(&mut s, "edit.undo", json!({}));
    assert!(!s.library().unwrap().is_trashed(page_id));
    // A name with nothing visible is refused.
    let e = s
        .dispatch(
            "list.entry.addPlaceholder",
            &json!({ "list": list, "label": "\u{200b}\u{202e}" }),
        )
        .unwrap_err();
    assert_eq!(e.code(), "bad_params");
    let _ = ids;
}

#[test]
fn perf_15_a_library_page_range_comes_with_the_score_once() {
    let (mut s, ids) = library("libraryrange", &[5]);
    run(
        &mut s,
        "score.pageRange.set",
        json!({ "score": ids[0], "first": 2, "last": 4 }),
    );
    let list = run(&mut s, "list.new", json!({ "name": "Audition" }))["list"]
        .as_str()
        .unwrap()
        .to_owned();
    let r = run(
        &mut s,
        "list.entry.add",
        json!({ "list": list, "score": ids[0] }),
    );
    assert_eq!(
        r["pageRange"],
        json!({ "first": 2, "last": 4, "fromLibrary": true })
    );
    let entry = r["entry"].as_str().unwrap().to_owned();
    // Changing either range leaves the other alone.
    run(
        &mut s,
        "list.entry.pageRange.set",
        json!({ "list": list, "entry": entry, "first": 3, "last": 3 }),
    );
    run(
        &mut s,
        "score.pageRange.set",
        json!({ "score": ids[0], "first": 1, "last": 5 }),
    );
    let got = run(&mut s, "list.get", json!({ "list": list }));
    assert_eq!(
        got["entries"][0]["pageRange"],
        json!({ "first": 2, "last": 2 })
    );
}
