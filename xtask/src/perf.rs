//! `cargo xtask perf [--update-baseline]`: measures the render budgets in `perf/budgets.toml`
//! with `crates/pdf/examples/perf.rs` and `crates/engine/examples/perf.rs` (release builds, the
//! corpus if fetched) and prints them
//! beside their budgets. `--update-baseline` writes `perf/baseline.json`, which
//! `cargo xtask scorecard` folds into docs/scorecard.md. Set `SCORECRAFT_PERF_DEVICE` to name
//! the machine; publish baselines only from a reference device.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;
use std::process::Command;

use serde_json::Value;

/// One measured scenario.
#[derive(Debug, Clone, PartialEq)]
pub struct Measured {
    pub id: String,
    pub ms: f64,
    pub samples: u64,
    pub source: String,
}

/// Parses the example's JSON lines; other output is ignored.
pub fn parse(output: &str) -> Vec<Measured> {
    output
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l.trim()).ok())
        .filter_map(|v| {
            Some(Measured {
                id: v.get("id")?.as_str()?.to_owned(),
                ms: v.get("ms")?.as_f64()?,
                samples: v.get("samples")?.as_u64()?,
                source: v.get("source")?.as_str()?.to_owned(),
            })
        })
        .collect()
}

/// `perf/baseline.json`, one `"P2": "84 ms",` line per scenario (the shape the scorecard reads).
pub fn baseline(device: &str, measured: &[Measured]) -> String {
    let mut s = String::from("{\n");
    let _ = writeln!(s, "  \"device\": {},", Value::from(device));
    let rows: Vec<String> = measured
        .iter()
        .map(|m| {
            format!(
                "  \"{}\": \"{:.0} ms ({}, {} samples)\"",
                m.id, m.ms, m.source, m.samples
            )
        })
        .collect();
    s.push_str(&rows.join(",\n"));
    s.push_str("\n}\n");
    s
}

pub fn run(root: &Path, update_baseline: bool) -> Result<(), String> {
    let corpus = root.join("corpus/public-domain");
    if !corpus.is_dir() {
        println!("perf: no corpus (run `cargo xtask corpus --all`); using synthetic scores");
    }
    let mut stdout = String::new();
    // Render budgets from the PDF adapter; the turn budget (P1) from the engine.
    for (package, example) in [
        ("scorecraft-pdf", "perf"),
        ("scorecraft-engine", "reader_perf"),
        ("scorecraft-engine", "library_perf"),
    ] {
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
        let mut cmd = Command::new(cargo);
        cmd.current_dir(root).args([
            "run",
            "--release",
            "-q",
            "-p",
            package,
            "--example",
            example,
        ]);
        if corpus.is_dir() {
            cmd.arg("--").arg(&corpus);
        }
        let out = cmd.output().map_err(|e| format!("running cargo: {e}"))?;
        if !out.status.success() {
            return Err(format!(
                "the {package} perf example failed:\n{}",
                String::from_utf8_lossy(&out.stderr)
            ));
        }
        stdout.push_str(&String::from_utf8_lossy(&out.stdout));
    }
    let measured = parse(&stdout);
    let budgets = crate::scorecard::parse_budgets(
        &fs::read_to_string(root.join("perf/budgets.toml")).map_err(|e| e.to_string())?,
    )?;
    for m in &measured {
        let budget = budgets
            .iter()
            .find(|b| b.id == m.id)
            .map_or("?", |b| b.budget.as_str());
        println!(
            "perf: {:<4} {:>8.1} ms  budget {budget:<10} ({} samples, {})",
            m.id, m.ms, m.samples, m.source
        );
    }
    if update_baseline {
        let device =
            std::env::var("SCORECRAFT_PERF_DEVICE").unwrap_or_else(|_| "unnamed device".to_owned());
        fs::write(
            root.join("perf/baseline.json"),
            baseline(&device, &measured),
        )
        .map_err(|e| format!("writing perf/baseline.json: {e}"))?;
        println!("perf: wrote perf/baseline.json; run `cargo xtask scorecard`");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_example_lines_and_skips_noise() {
        let out =
            "warming up\n{\"id\":\"P2\",\"ms\":84.1,\"samples\":12,\"source\":\"corpus\"}\n{bad\n";
        let m = parse(out);
        assert_eq!(m.len(), 1);
        assert_eq!((m[0].id.as_str(), m[0].samples), ("P2", 12));
    }

    #[test]
    fn baseline_is_json_the_scorecard_can_read() {
        let m = parse("{\"id\":\"P13\",\"ms\":212.6,\"samples\":20,\"source\":\"corpus\"}");
        let text = baseline("HP \"Spectre\" x360", &m);
        let v: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["P13"], "213 ms (corpus, 20 samples)");
        assert!(text.lines().any(|l| l.trim().starts_with("\"P13\":")));
    }
}
