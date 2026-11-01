//! Perf scenarios P6 and P7 (`perf/budgets.toml`, spec `docs/spec/library.md` LIB-13): a library
//! of 10,000 scores opened cold and listed (P6), and metadata searches across it (P7). Prints one
//! JSON line per scenario, as the other perf examples do; `cargo xtask perf` runs it.
//!
//! The library is built once in a temporary folder from small synthetic PDFs with varied titles
//! and composers, then closed and reopened, so P6 measures reading it from disk.

use std::time::Instant;

use scorecraft_engine::Session;
use scorecraft_testkit::pdf::{PageSpec, PdfBuilder};
use serde_json::json;

const SCORES: usize = 10_000;
const BATCH: usize = 250;

const COMPOSERS: [&str; 10] = [
    "J. S. Bach",
    "W. A. Mozart",
    "L. van Beethoven",
    "F. Chopin",
    "C. Debussy",
    "A. Dvořák",
    "G. Fauré",
    "J. Brahms",
    "F. Schubert",
    "M. Ravel",
];
const FORMS: [&str; 8] = [
    "Sonata",
    "Étude",
    "Nocturne",
    "Prelude",
    "Fugue",
    "Waltz",
    "Rondo",
    "Impromptu",
];

fn median(mut v: Vec<f64>) -> Option<f64> {
    v.sort_by(f64::total_cmp);
    v.get(v.len() / 2).copied()
}

fn main() {
    let n = std::env::var("SCORECRAFT_PERF_SCORES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(SCORES);
    let dir = std::env::temp_dir().join(format!("scorecraft-library-perf-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let files = dir.join("files");
    if std::fs::create_dir_all(&files).is_err() {
        eprintln!("perf: can't make {}", files.display());
        return;
    }
    let lib = dir
        .join("Library.scorecraft")
        .to_string_lossy()
        .into_owned();
    let mut s = Session::new();
    if s.dispatch("library.create", &json!({ "path": lib }))
        .is_err()
    {
        eprintln!("perf: can't create the library");
        return;
    }
    let mut paths = Vec::new();
    for i in 0..n {
        let form = FORMS.get(i % FORMS.len()).copied().unwrap_or("Piece");
        let title = format!("{form} No. {}", i / FORMS.len() + 1);
        let composer = COMPOSERS.get(i % COMPOSERS.len()).copied().unwrap_or("");
        let bytes = PdfBuilder::new()
            .pages(1, PageSpec::default())
            .title(&title)
            .composer(composer)
            .build();
        let p = files.join(format!("s{i}.pdf"));
        if std::fs::write(&p, bytes).is_err() {
            eprintln!("perf: can't write {}", p.display());
            return;
        }
        paths.push(p.to_string_lossy().into_owned());
        if paths.len() == BATCH || i + 1 == n {
            let r = s.dispatch(
                "library.import.files",
                &json!({ "paths": std::mem::take(&mut paths) }),
            );
            if let Err(e) = r {
                eprintln!("perf: import failed: {e}");
                return;
            }
        }
    }
    let _ = s.dispatch("library.close", &json!({}));
    let source = format!("synthetic, {n} scores");

    // P6: cold open to the first screen of the grid.
    let mut open = Vec::new();
    for _ in 0..3 {
        let mut fresh = Session::new();
        let t = Instant::now();
        let ok = fresh
            .dispatch("library.open", &json!({ "path": lib }))
            .and_then(|_| fresh.dispatch("library.search", &json!({ "query": "", "limit": 60 })))
            .is_ok();
        if ok {
            open.push(t.elapsed().as_secs_f64() * 1000.0);
        }
        s = fresh;
    }
    let k = open.len();
    if let Some(ms) = median(open) {
        println!(r#"{{"id":"P6","ms":{ms:.2},"samples":{k},"source":"{source}"}}"#);
    }

    // P7: metadata searches.
    let mut search = Vec::new();
    for q in [
        "moz son",
        "dvorak",
        "etude 12",
        "bach fugue",
        "x",
        "ravel waltz no 40",
        "faure",
        "sonata",
    ] {
        let t = Instant::now();
        if s.dispatch("library.search", &json!({ "query": q })).is_ok() {
            search.push(t.elapsed().as_secs_f64() * 1000.0);
        }
    }
    let k = search.len();
    if let Some(ms) = median(search) {
        println!(r#"{{"id":"P7","ms":{ms:.2},"samples":{k},"source":"{source}"}}"#);
    }
    let _ = std::fs::remove_dir_all(&dir);
}
