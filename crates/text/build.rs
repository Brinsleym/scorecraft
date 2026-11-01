//! Embeds optional fonts from `SCORECRAFT_FONTS_DIR` (docs/standards/fonts.md). Fonts are never
//! committed: with the variable unset, or a font missing from the checkout, the generated
//! constant is `None` and `scorecraft-text` falls back to its built-in face.

use std::path::{Path, PathBuf};

/// Where Edwin Bold Italic may sit in a fonts checkout (MuseScoreFonts/Edwin, SIL OFL 1.1).
const EDWIN_BOLD_ITALIC: &[&str] = &[
    "Edwin/Edwin-BdIta.otf",
    "edwin/Edwin-BdIta.otf",
    "Edwin-BdIta.otf",
];

fn find(dir: &Path, candidates: &[&str]) -> Option<PathBuf> {
    candidates.iter().map(|c| dir.join(c)).find(|p| p.is_file())
}

fn main() {
    println!("cargo:rerun-if-env-changed=SCORECRAFT_FONTS_DIR");
    let Some(out) = std::env::var_os("OUT_DIR") else {
        return;
    };
    let edwin = std::env::var_os("SCORECRAFT_FONTS_DIR")
        .map(PathBuf::from)
        .and_then(|dir| find(&dir, EDWIN_BOLD_ITALIC));
    let line = match &edwin {
        Some(path) => {
            println!("cargo:rerun-if-changed={}", path.display());
            format!(
                "pub(crate) static EDWIN_BOLD_ITALIC: Option<&[u8]> = Some(include_bytes!({:?}));\n",
                path.display().to_string()
            )
        }
        None => "pub(crate) static EDWIN_BOLD_ITALIC: Option<&[u8]> = None;\n".to_owned(),
    };
    let _ = std::fs::write(Path::new(&out).join("fonts.rs"), line);
}
