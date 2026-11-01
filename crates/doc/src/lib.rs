//! scorecraft-doc: the library model. Scores, page refs, annotations, layers, bookmarks,
//! setlists, folders and Recently Deleted, as pure data.
//!
//! Layer L1 in the workspace map (`docs/architecture.md` §2–3, §5). No I/O, no clock, no RNG:
//! ids come from an [`IdGen`] the caller seeds, times are Unix milliseconds the caller reads
//! (through `chrono`, rule 10). `scorecraft-ops` edits this model, `scorecraft-format` stores it.
//!
//! Every type is `serde` with defaults for optional fields, so an older library file loads into
//! a newer model. Adding a field here must also map it in `scorecraft-format`, which fails to
//! compile until it does (architecture §8).
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

mod annotation;
mod blob;
mod color;
mod hlc;
mod ids;
mod library;
mod score;
mod validate;

pub use annotation::{
    Annotation, AnnotationKind, Hairpin, InkPoint, Nib, PageArea, PageKey, Shortcut, Snip, Stamp,
    Stroke, TextNote, TextStyle, Tool,
};
pub use blob::BlobRef;
pub use color::Color;
pub use hlc::Hlc;
pub use ids::{
    AnnotationId, BookmarkId, DeviceId, EntryId, FolderId, IdGen, LayerId, LibraryId, ListId,
    ScoreId,
};
pub use library::{EntryItem, Folder, Library, ListEntry, Setlist};
pub use score::{
    AudioTrack, Bookmark, Layer, Metadata, MidiOnOpen, PageRange, PageRef, Score, Source,
    SourceKind, TurnMark,
};
pub use validate::{MAX_PROBLEMS, Problem};

/// The crate's package name, used by `cargo xtask layers` smoke tests and diagnostics.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

#[cfg(test)]
mod tests {
    #[test]
    fn crate_name_matches_package() {
        assert_eq!(super::CRATE_NAME, "scorecraft-doc");
    }
}
