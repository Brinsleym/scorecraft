//! Fonts that come with PDFCraft's crates, for other crates that draw text (text notes in
//! `scorecraft-text`). Plain bytes cross this door, never a PDFCraft type.

/// Inter Regular (SIL OFL 1.1, listed in ATTRIBUTION.md), embedded by `pdfcraft-fonts`: the
/// built-in sans-serif face, so text always draws even without a fonts checkout.
pub fn interface_font() -> &'static [u8] {
    pdfcraft_fonts::INTER_REGULAR
}

#[cfg(test)]
mod tests {
    #[test]
    fn inter_is_an_sfnt() {
        let bytes = super::interface_font();
        assert!(bytes.len() > 10_000);
        assert_eq!(bytes.get(..4), Some(&[0, 1, 0, 0][..]));
    }
}
