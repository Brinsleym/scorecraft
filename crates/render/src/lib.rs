//! scorecraft-render: CPU page compositor (the oracle): PDF raster, display filters, ink and snips.
//!
//! Today it holds the page cache that makes a turn a cache hit (`docs/architecture.md` §7):
//! bitmaps keyed by page, [`ZoomBucket`], [`Filter`] and region in a byte-budget LRU
//! ([`TileCache`]), and a [`ScoreView`] that renders on a miss and pre-renders the
//! [`neighbours`] of the current page on a background thread.
//!
//! [`ink`] draws a page's strokes onto its bitmap: the CPU oracle for ink (spec
//! `docs/spec/ink.md`).
//!
//! Layer L3 in the workspace map (`docs/architecture.md` §2–3).
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

mod cache;
mod filter;
pub mod ink;
mod view;
mod zoom;

pub use cache::{CacheStats, Region, TileCache, TileKey};
pub use filter::Filter;
pub use ink::PageInk;
pub use view::{ScoreView, TILE_SIDE, neighbours};
pub use zoom::ZoomBucket;

/// The crate's package name, used by `cargo xtask layers` smoke tests and diagnostics.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

/// Default bitmap budget for one open score: about 18 full-height letter pages on a display
/// 2160 pixels tall.
pub const DEFAULT_BUDGET: usize = 256 * 1024 * 1024;

#[cfg(test)]
mod tests {
    #[test]
    fn crate_name_matches_package() {
        assert_eq!(super::CRATE_NAME, "scorecraft-render");
    }
}
