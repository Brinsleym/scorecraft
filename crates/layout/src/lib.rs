//! scorecraft-layout: Page layout: single, two-page, half-page and alternating turns, zoom, crop and snips.
//!
//! Layer L2 in the workspace map (`docs/architecture.md` §2–3). Pure geometry, no rendering: the
//! [`Reader`] holds the layout, turn mode and position in a score, turns forward and back
//! ([`docs/spec/page-turning.md`](../../../docs/spec/page-turning.md)), and [`place`] maps the
//! current [`View`] to rectangles on screen that a frontend fills with page bitmaps.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

mod place;
mod reader;
mod snap;

pub use place::{Fit, Frame, PageSize, Slot, Viewport, ZOOM_RANGE, Zoom, place};
pub use reader::{Layout, Reader, SPLIT_RANGE, TurnMode, View, views};
pub use scorecraft_geom::{Rect, Size};
pub use snap::{MAX_SNAP, snap_split, system_gaps};

/// The crate's package name, used by `cargo xtask layers` smoke tests and diagnostics.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

#[cfg(test)]
mod tests {
    #[test]
    fn crate_name_matches_package() {
        assert_eq!(super::CRATE_NAME, "scorecraft-layout");
    }
}
