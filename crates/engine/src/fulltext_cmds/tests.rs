//! Full-text search (LIB-6; `docs/spec/library.md`, LIBS-11, LIBS-12).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use serde_json::{Value, json};

use scorecraft_testkit::pdf::{PageSpec, PdfBuilder};

use crate::Session;

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    s.dispatch(id, &p)
        .unwrap_or_else(|e| panic!("{id} {p}: {e} ({})", e.code()))
}

fn library(name: &str) -> (Session, Vec<String>) {
    let dir = std::env::temp_dir().join(format!("scorecraft-ft-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut s = Session::new();
    s.set_clock(Box::new(|| 1_793_404_800_000));
    s.create_memory_library().unwrap();
    let mut ids = Vec::new();
    for (i, pages) in [3usize, 2].iter().enumerate() {
        let path = dir.join(format!("{i}.pdf"));
        std::fs::write(
            &path,
            PdfBuilder::new()
                .pages(*pages, PageSpec::default())
                .title(&format!("Score {i}"))
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

#[test]
fn libs_11_words_inside_the_pdf_are_found_with_their_page() {
    let (mut s, _) = library("pdf");
    // Each page prints its number: only the 3-page score has a "3".
    let r = run(&mut s, "library.search.fullText", json!({ "query": "3" }));
    assert_eq!(
        (r["count"].as_u64(), r["pending"].as_u64()),
        (Some(1), Some(0)),
        "{r}"
    );
    assert_eq!(r["scores"][0]["title"], "Score 0");
    assert_eq!(r["scores"][0]["filePage"], 3);
    assert_eq!(r["scores"][0]["snippet"], "3");
    let both = run(&mut s, "library.search.fullText", json!({ "query": "2" }));
    assert_eq!(both["count"], 2);
    // The index survives losing the session's memory: it is read back from the library's cache.
    s.text_index.clear();
    let again = run(&mut s, "library.search.fullText", json!({ "query": "3" }));
    assert_eq!(again["count"], 1);
    assert!(
        s.dispatch("library.search.fullText", &json!({ "query": "" }))
            .is_err()
    );
}

#[test]
fn libs_12_practice_notes_are_searched_too() {
    let (mut s, ids) = library("notes");
    run(
        &mut s,
        "score.metadata.set",
        json!({ "score": ids[1], "notes": "Take the coda with rubato" }),
    );
    let r = run(
        &mut s,
        "library.search.fullText",
        json!({ "query": "RUBATO" }),
    );
    assert_eq!(r["count"], 1);
    assert_eq!(r["scores"][0]["title"], "Score 1");
    assert!(r["scores"][0]["notes"].as_str().unwrap().contains("rubato"));
}
