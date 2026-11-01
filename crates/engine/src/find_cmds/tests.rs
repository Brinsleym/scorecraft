//! Search, sort, recent and folder import (`docs/spec/library.md`, LIBS-1..8).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use scorecraft_testkit::pdf::{PageSpec, PdfBuilder};

use crate::Session;

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    s.dispatch(id, &p)
        .unwrap_or_else(|e| panic!("{id} {p}: {e} ({})", e.code()))
}

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("scorecraft-find-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A distinct one-page PDF (`pages` varies the bytes).
fn pdf(dir: &Path, name: &str, pages: usize) -> String {
    let path = dir.join(name);
    std::fs::write(
        &path,
        PdfBuilder::new().pages(pages, PageSpec::default()).build(),
    )
    .unwrap();
    path.to_string_lossy().into_owned()
}

/// A library with scores titled and composed as given; returns their ids. The clock is
/// shared so tests can move time on.
fn library(name: &str, scores: &[(&str, &str)]) -> (Session, Vec<String>, Arc<Mutex<u64>>) {
    let dir = temp(name);
    let clock = Arc::new(Mutex::new(1_793_404_800_000u64));
    let c = clock.clone();
    let mut s = Session::new();
    s.set_clock(Box::new(move || *c.lock().unwrap()));
    s.create_memory_library().unwrap();
    let mut ids = Vec::new();
    for (i, (title, composer)) in scores.iter().enumerate() {
        let path = pdf(&dir, &format!("s{i}.pdf"), i + 1);
        let r = run(&mut s, "library.import.files", json!({ "paths": [path] }));
        let id = r["imported"][0]["score"].as_str().unwrap().to_owned();
        run(
            &mut s,
            "score.metadata.set",
            json!({ "score": id, "title": title, "composer": composer }),
        );
        ids.push(id);
    }
    (s, ids, clock)
}

fn titles(v: &Value) -> Vec<String> {
    v["scores"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["title"].as_str().unwrap().to_owned())
        .collect()
}

fn find(s: &mut Session, q: &str) -> Vec<String> {
    titles(&run(s, "library.search", json!({ "query": q })))
}

#[test]
fn libs_1_2_search_matches_word_starts_without_accents_and_ranks_titles_first() {
    let (mut s, _, _) = library(
        "search",
        &[
            ("Sonata in A", "W. A. Mozart"),
            ("Étude", "Chopin"),
            ("Rondo", "Weber"),
            ("Air", "Rondo Society"),
        ],
    );
    assert_eq!(find(&mut s, "moz son"), ["Sonata in A"]);
    assert_eq!(find(&mut s, "etude"), ["Étude"]);
    assert!(find(&mut s, "chopin xyz").is_empty());
    assert_eq!(find(&mut s, "rondo"), ["Rondo", "Air"]);
    assert_eq!(
        find(&mut s, "").len(),
        4,
        "an empty search lists everything"
    );
    let r = run(&mut s, "library.search", json!({ "query": "", "limit": 2 }));
    assert_eq!((r["count"].as_u64(), titles(&r).len()), (Some(4), 2));
}

#[test]
fn libs_3_long_searches_are_refused() {
    let (mut s, _, _) = library("long", &[("A", "B")]);
    for q in ["x".repeat(300), vec!["w"; 17].join(" ")] {
        let e = s
            .dispatch("library.search", &json!({ "query": q }))
            .unwrap_err();
        assert_eq!(e.code(), "bad_params");
    }
}

#[test]
fn libs_4_5_sorting_by_title_composer_added_and_opened() {
    let (mut s, ids, clock) = library(
        "sort",
        &[
            ("Etude 10", "Chopin"),
            ("Etude 2", "Liszt"),
            ("The Swan", "Saint-Saëns"),
            ("apple", "Bach"),
        ],
    );
    let list = |s: &mut Session| titles(&run(s, "library.scores", json!({})));
    assert_eq!(list(&mut s), ["apple", "Etude 2", "Etude 10", "The Swan"]);
    run(&mut s, "library.sort.set", json!({ "by": "composer" }));
    assert_eq!(list(&mut s), ["apple", "Etude 10", "Etude 2", "The Swan"]);
    run(&mut s, "library.sort.set", json!({ "descending": true }));
    assert_eq!(list(&mut s), ["The Swan", "Etude 2", "Etude 10", "apple"]);
    // Open three in turn, a minute apart; "apple" is never opened.
    for i in [1usize, 0, 2] {
        *clock.lock().unwrap() += 60_000;
        run(&mut s, "score.open", json!({ "score": ids[i] }));
    }
    let r = run(
        &mut s,
        "library.sort.set",
        json!({ "by": "opened", "descending": false }),
    );
    assert_eq!(r["sort"]["by"], "opened");
    assert_eq!(list(&mut s), ["The Swan", "Etude 10", "Etude 2", "apple"]);
    let e = s
        .dispatch("library.sort.set", &json!({ "by": "colour" }))
        .unwrap_err();
    assert_eq!(e.code(), "bad_params");
}

#[test]
fn libs_6_the_recent_list_leaves_out_the_trash() {
    let (mut s, ids, clock) = library("recent", &[("One", ""), ("Two", ""), ("Three", "")]);
    for id in &ids[..2] {
        *clock.lock().unwrap() += 1_000;
        run(&mut s, "score.open", json!({ "score": id }));
    }
    assert_eq!(
        titles(&run(&mut s, "library.recent.list", json!({}))),
        ["Two", "One"]
    );
    run(&mut s, "library.score.delete", json!({ "score": ids[1] }));
    assert_eq!(
        titles(&run(&mut s, "library.recent.list", json!({ "limit": 5 }))),
        ["One"]
    );
}

#[test]
fn libs_7_a_folder_imports_once_with_its_subfolders() {
    let dir = temp("folder-src");
    let a = pdf(&dir, "a.pdf", 1);
    pdf(&dir, "B.PDF", 2);
    std::fs::write(dir.join("notes.txt"), "not a score").unwrap();
    let sub = dir.join("Bach");
    std::fs::create_dir_all(&sub).unwrap();
    pdf(&sub, "c.pdf", 3);
    let mut s = Session::new();
    s.set_clock(Box::new(|| 1_793_404_800_000));
    s.create_memory_library().unwrap();
    run(&mut s, "library.import.files", json!({ "paths": [a] }));
    let r = run(
        &mut s,
        "library.import.folder",
        json!({ "path": dir.to_string_lossy(), "keepFolders": true }),
    );
    assert_eq!(r["imported"].as_array().unwrap().len(), 2, "{r}");
    assert_eq!(
        (r["skipped"].as_u64(), r["ignored"].as_u64()),
        (Some(1), Some(1))
    );
    assert_eq!(r["folders"], 1);
    // LIBS-8: a score filed in folder "Bach" is found by its folder's name.
    let found = run(&mut s, "library.search", json!({ "query": "bach" }));
    assert_eq!(found["count"], 1);
    assert!(found["scores"][0]["folder"].is_string());
    assert_eq!(
        run(&mut s, "library.scores", json!({}))["scores"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(
        run(&mut s, "library.scores", json!({}))["scores"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let e = s
        .dispatch(
            "library.import.folder",
            &json!({ "path": dir.join("a.pdf").to_string_lossy() }),
        )
        .unwrap_err();
    assert_eq!(e.code(), "io_error");
}
