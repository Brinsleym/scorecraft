//! Repository tasks: `cargo xtask <task>`. See `docs/development.md`.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

mod ci;
mod corpus;
mod graph;
mod ico;
mod layers;
mod parity;
mod perf;
mod scorecard;
mod wasm;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "usage: cargo xtask <task>

tasks:
  ci       run fmt, clippy, tests, layers and wasm, as the Linux CI jobs do
  corpus   the pinned public-domain score corpus: --all downloads and verifies it,
           --references renders PDFium references (needs Python 3 with pypdfium2 and pillow);
           takes --library and --max-pages like test-corpus
  ico      pack assets/app-icon/hicolor's PNGs into assets/app-icon/scorecraft.ico
  layers   check crate layering, lints and crate-root attributes (docs/architecture.md §3)
  parity   regenerate the feature catalog and docs/parity-checklist.md (--check: fail if stale)
  scorecard regenerate docs/scorecard.md (--check: fail if stale)
  perf     measure the render budgets (perf/budgets.toml); --update-baseline writes
           perf/baseline.json
  test-corpus  fetch the corpus and run the opt-in corpus test (PDF-1, PDF-2);
           --library <dir> tests a private folder instead (read only, results in corpus/),
           --max-pages <n> checks the first n pages of each score
  wasm     cargo check every L0–L6 crate for wasm32-unknown-unknown";

fn workspace_root() -> PathBuf {
    // xtask lives one level below the workspace root.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = workspace_root();
    let result = match args.first().map(String::as_str) {
        Some("ci") => ci::run(&root),
        Some("corpus") => corpus::run(&root, &args),
        Some("ico") => ico::run(&root),
        Some("layers") => layers::run(&root),
        Some("perf") => perf::run(&root, args.iter().any(|a| a == "--update-baseline")),
        Some("test-corpus") => corpus::test(&root, &args),
        Some("parity") => parity::run(&root, args.iter().any(|a| a == "--check")),
        Some("scorecard") => scorecard::run(&root, args.iter().any(|a| a == "--check")),
        Some("wasm") => wasm::run(&root),
        Some("-h" | "--help" | "help") | None => {
            println!("{USAGE}");
            Ok(())
        }
        Some(other) => Err(format!("unknown task `{other}`\n\n{USAGE}")),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("xtask: {e}");
            ExitCode::FAILURE
        }
    }
}
