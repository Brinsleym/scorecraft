//! Perf scenario P1 (`perf/budgets.toml`): a turn to a pre-rendered page, from the turn command to
//! every bitmap of the new view being ready to draw. Prints one JSON line, as
//! `crates/pdf/examples/perf.rs` does; `cargo xtask perf` runs both.
//!
//! With a path argument the first PDF in that directory is used, else a synthetic score. The
//! score is imported into a memory library and every page is marked up (pen strokes and a
//! highlighter, as a heavily marked part), so the turn includes drawing ink onto the new pages.

use std::sync::Arc;
use std::time::{Duration, Instant};

use scorecraft_engine::Session;
use scorecraft_testkit::pdf::{PageSpec, PdfBuilder};
use serde_json::json;

fn median(mut v: Vec<f64>) -> Option<f64> {
    v.sort_by(f64::total_cmp);
    v.get(v.len() / 2).copied()
}

fn score(dir: Option<&str>) -> Option<(Vec<u8>, &'static str)> {
    if let Some(dir) = dir {
        let mut pdfs: Vec<_> = std::fs::read_dir(dir)
            .ok()?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf")))
            .collect();
        pdfs.sort();
        for p in pdfs {
            if let Ok(bytes) = std::fs::read(&p) {
                return Some((bytes, "corpus"));
            }
        }
    }
    Some((
        PdfBuilder::new().pages(24, PageSpec::default()).build(),
        "synthetic",
    ))
}

/// Strokes per page: a heavily marked part.
const STROKES_PER_PAGE: usize = 30;

/// Imports `bytes` into a memory library, opens it, and marks every page.
fn marked_up(s: &mut Session, bytes: &[u8]) -> Result<(), String> {
    let dir = std::env::temp_dir().join(format!("scorecraft-perf-{}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join("perf.pdf");
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    s.create_memory_library().map_err(|e| e.to_string())?;
    let r = s
        .dispatch(
            "library.import.files",
            &json!({ "paths": [path.to_string_lossy()] }),
        )
        .map_err(|e| e.to_string())?;
    let _ = std::fs::remove_dir_all(&dir);
    let id = r
        .pointer("/imported/0/score")
        .and_then(serde_json::Value::as_str)
        .ok_or("the import failed")?
        .to_owned();
    let open = s
        .dispatch("score.open", &json!({ "score": id }))
        .map_err(|e| e.to_string())?;
    let pages = open
        .get("pages")
        .and_then(serde_json::Value::as_array)
        .map_or(0, Vec::len);
    for page in 1..=pages {
        for k in 0..STROKES_PER_PAGE {
            let y = 60.0 + k as f64 * 22.0;
            let pts: Vec<_> = (0..120)
                .map(|i| {
                    json!([
                        40.0 + f64::from(i) * 4.5,
                        y + (f64::from(i) * 0.4).sin() * 6.0,
                        0.7
                    ])
                })
                .collect();
            let tool = if k % 10 == 9 {
                "annotate.tool.highlighter"
            } else {
                "annotate.tool.pen"
            };
            s.dispatch(tool, &json!({})).map_err(|e| e.to_string())?;
            s.dispatch(
                "annotate.stroke",
                &json!({ "space": "page", "page": page, "points": pts }),
            )
            .map_err(|e| e.to_string())?;
        }
    }
    s.dispatch("view.page.goto", &json!({ "page": 1 }))
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn main() {
    let arg = std::env::args().nth(1);
    let Some((bytes, source)) = score(arg.as_deref()) else {
        eprintln!("perf: no score");
        return;
    };
    let mut s = Session::new();
    let source = match marked_up(&mut s, &bytes) {
        Ok(()) => format!("{source}+ink"),
        Err(e) => {
            eprintln!("perf: {e}; timing turns without ink");
            if let Err(e) = s.open_score_bytes(Arc::new(bytes), None, "perf.pdf") {
                eprintln!("perf: {e}");
                return;
            }
            source.to_owned()
        }
    };
    // A Windows tablet in landscape at 150% scaling.
    let _ = s.dispatch(
        "view.viewport.set",
        &json!({ "width": 1440, "height": 900, "pixelsPerPoint": 1.5 }),
    );
    let _ = s.reader_frame(true);
    let mut samples = Vec::new();
    for _ in 0..20 {
        s.wait_for_prefetch(Duration::from_secs(30));
        let t = Instant::now();
        let turned = s
            .dispatch("view.turn.next", &json!({}))
            .ok()
            .and_then(|r| r.get("turned").and_then(serde_json::Value::as_bool))
            .unwrap_or(false);
        let frame = s.reader_frame(false).unwrap_or_default();
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        if !turned {
            break;
        }
        if frame.iter().all(|f| f.bitmap.is_some()) {
            samples.push(ms);
        } else {
            eprintln!("perf: a turn missed the pre-render cache");
        }
    }
    let n = samples.len();
    if let Some(ms) = median(samples) {
        println!(r#"{{"id":"P1","ms":{ms:.2},"samples":{n},"source":"{source}"}}"#);
    }
}
