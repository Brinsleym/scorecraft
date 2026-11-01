//! scorecraft-web: ScoreCraft in the browser. The same shell as the desktop app (`ui-egui`), on
//! eframe's WebGL backend, in the page's `scorecraft_canvas`. Scores come from the file picker
//! or a drop and open through an in-memory library; settings are kept in local storage.
//!
//! Built with `wasm-bindgen` (see `index.html`); a native build of this crate only says how.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

#[cfg(target_arch = "wasm32")]
mod idb;
#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(target_arch = "wasm32")]
fn main() {
    web::start();
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> std::process::ExitCode {
    println!(
        "{} {}: build for wasm32-unknown-unknown and serve index.html (see the crate docs)",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION")
    );
    std::process::ExitCode::SUCCESS
}
