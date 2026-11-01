//! scorecraft-text: fonts, measuring, wrapping and glyph outlines for text notes (ANNO-8) and
//! stamps (ANNO-9). Spec: `docs/spec/ink.md`.
//!
//! Layer L2 in the workspace map (`docs/architecture.md` §2–3). Faces are Inter Regular, built in
//! through `scorecraft-pdf` (PDFCraft embeds it), and Edwin Bold Italic for dynamics when the build
//! has a fonts checkout (`SCORECRAFT_FONTS_DIR`, docs/standards/fonts.md); without Edwin, dynamics
//! use Inter slanted and emboldened. Glyph outlines come out in page space (points, y up) as
//! closed polygons for the non-zero fill every renderer uses.
//!
//! Text comes from players and agents: lengths are capped ([`MAX_TEXT_CHARS`]), unknown
//! characters draw as the font's missing-glyph box, and nothing panics.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

mod face;
mod layout;

pub use face::{Face, FaceKind};
pub use layout::{LINE_SPACING, Laid, MAX_TEXT_CHARS, NOTE_MAX_WIDTH, fit_inside, layout, wrap};

/// The crate's package name, used by `cargo xtask layers` smoke tests and diagnostics.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

#[cfg(test)]
mod tests {
    #[test]
    fn crate_name_matches_package() {
        assert_eq!(super::CRATE_NAME, "scorecraft-text");
    }
}
