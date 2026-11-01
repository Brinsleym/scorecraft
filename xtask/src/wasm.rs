//! `cargo xtask wasm`: every L0–L6 crate must build for `wasm32-unknown-unknown` (AGENTS.md rule 7),
//! and so must the browser app, `scorecraft-web`.

use std::path::Path;
use std::process::Command;

use crate::layers::{CRATES, Layer};

pub const TARGET: &str = "wasm32-unknown-unknown";

/// The crates the web build depends on: every library crate in L0–L6.
pub fn wasm_crates() -> Vec<&'static str> {
    CRATES
        .iter()
        .filter(|(_, layer)| matches!(layer, Layer::L(n) if *n <= 6))
        .map(|(name, _)| *name)
        .collect()
}

pub fn run(root: &Path) -> Result<(), String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let mut cmd = Command::new(cargo);
    cmd.current_dir(root).args(["check", "--target", TARGET]);
    for name in wasm_crates() {
        cmd.args(["-p", name]);
    }
    // The browser app itself (its web code compiles only for wasm32).
    cmd.args(["-p", "scorecraft-web"]);
    let status = cmd.status().map_err(|e| format!("running cargo: {e}"))?;
    if status.success() {
        println!(
            "wasm: {} crates and the browser app check for {TARGET}",
            wasm_crates().len()
        );
        Ok(())
    } else {
        Err(format!(
            "cargo check --target {TARGET} failed (install it with `rustup target add {TARGET}`)"
        ))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn covers_l0_to_l6_only() {
        let crates = super::wasm_crates();
        assert!(crates.contains(&"scorecraft-geom"));
        assert!(crates.contains(&"scorecraft-ui-egui"));
        assert!(!crates.contains(&"scorecraft-testkit"));
        assert!(!crates.contains(&"scorecraft"));
        assert!(!crates.contains(&"xtask"));
    }
}
