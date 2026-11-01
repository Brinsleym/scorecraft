//! scorecraft-pdf: the one door to PDFCraft's PDF crates (`docs/architecture.md` §15).
//!
//! Opens a score, reports its pages, metadata and outline, rasterises pages and tiles, and
//! extracts text, and hands out the font bytes PDFCraft embeds. No `pdfcraft-*` type appears in this API, so the backend can be bumped, forked
//! or replaced by touching this crate only. Every call returns a [`PdfError`] instead of
//! panicking, whatever the input.
//!
//! Layer L0, standalone: no workspace dependencies.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

mod document;
mod error;
pub mod export;
mod fonts;

pub use document::{
    Bitmap, Document, MAX_PIXELS, MAX_SIDE, Metadata, OutlineEntry, PageGeometry, PageText,
    PaperColors, TextGlyph, Tile, device_pixels, fit_scale,
};
pub use error::PdfError;
pub use export::{ExportMark, ExportMode, ExportPage, InkFill, MarkKind, export_pdf};
pub use fonts::interface_font;

/// The crate's package name, used in diagnostics.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

/// Points per inch in PDF user space (ISO 32000-1 §8.3.2.3): `scale = dpi / POINTS_PER_INCH`.
pub const POINTS_PER_INCH: f32 = 72.0;

#[cfg(test)]
mod tests {
    #[test]
    fn crate_name_matches_package() {
        assert_eq!(super::CRATE_NAME, "scorecraft-pdf");
    }
}
