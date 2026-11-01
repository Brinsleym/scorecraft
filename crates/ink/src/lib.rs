//! scorecraft-ink: the stroke engine. Cleans and smooths pen samples, maps pressure to width,
//! builds the filled outline every renderer draws, hit-tests strokes for the erasers and the
//! rectangle tool, rules strokes along the ruler and straightens shapes (spec `docs/spec/ink.md`).
//!
//! Layer L2 in the workspace map (`docs/architecture.md` §2–3). Pure and deterministic: no I/O,
//! no clock, no randomness. Everything works in source-page PDF space (points, y up), so the same
//! outline serves the reader, export and every zoom.
//!
//! Samples come from devices and agents, so every function takes hostile input: non-finite
//! numbers are dropped, sizes are capped ([`MAX_POINTS`], [`MAX_OUTLINE_POINTS`]) and nothing
//! panics.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

mod erase;
mod error;
mod outline;
mod prepare;
mod pressure;
mod ruler;
mod shape;
mod smooth;
mod style;

pub use erase::{Erased, erase, inside_rect, touches};
pub use error::InkError;
pub use outline::{Outline, chisel_half_width, outline};
pub use prepare::{MAX_COORD, MAX_POINTS, MIN_SPACING, prepare};
pub use pressure::PressureCurve;
pub use ruler::{RULER_SNAP_DEGREES, Ruler};
pub use shape::{LINE_SLACK, LINE_TOLERANCE, MIN_SHAPE_SIZE, Shape, straighten};
pub use smooth::{CORNER_DEGREES, MAX_OUTLINE_POINTS, Sample, smooth};
pub use style::{
    ERASER_WIDTHS, HIGHLIGHTER_PALETTE, HIGHLIGHTER_SWATCHES, HIGHLIGHTER_WIDTHS, PALETTE,
    PEN_WIDTHS, SWATCHES, ToolStyle, WHITEOUT_SWATCH, WHITEOUT_WIDTHS, WidthRange,
};

/// The crate's package name, used by `cargo xtask layers` smoke tests and diagnostics.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

#[cfg(test)]
mod tests {
    #[test]
    fn crate_name_matches_package() {
        assert_eq!(super::CRATE_NAME, "scorecraft-ink");
    }
}
