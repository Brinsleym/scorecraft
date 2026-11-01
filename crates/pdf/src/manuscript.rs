//! Blank manuscript paper (feature-map PAGE-6): pages of staves, grand staves or tablature,
//! drawn as plain lines into a new PDF on PDFCraft's object layer.

use pdfcraft_cos::{Dict, Document as Cos, Object, PdfString, SaveOptions, Stream};

use crate::PdfError;

/// What the staves are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Paper {
    /// Five-line staves, one after another.
    Single,
    /// Pairs of staves joined at the left by a system line, for piano.
    Grand,
    /// Six-line guitar tablature.
    Tab,
}

impl Paper {
    pub fn from_id(id: &str) -> Option<Paper> {
        match id {
            "single" => Some(Paper::Single),
            "grand" => Some(Paper::Grand),
            "tab" => Some(Paper::Tab),
            _ => None,
        }
    }
}

/// The most pages one manuscript has.
pub const MAX_MANUSCRIPT_PAGES: usize = 200;

const MARGIN: f64 = 54.0;
/// Space between staff lines (a staff is 4 spaces high) and between tablature lines.
const SPACE: f64 = 7.0;
const TAB_SPACE: f64 = 8.0;
/// Space between staves, between grand-staff halves, and between grand staves.
const STAFF_GAP: f64 = 40.0;
const GRAND_INNER: f64 = 28.0;
const GRAND_GAP: f64 = 52.0;

fn n(v: f64) -> String {
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// The lines of one page, `width × height` points, as a content stream.
fn page_content(paper: Paper, width: f64, height: f64) -> String {
    let (x0, x1) = (MARGIN, width - MARGIN);
    let mut ops = String::from("q 0 G 0.6 w\n");
    let line = |y: f64, ops: &mut String| {
        ops.push_str(&format!("{} {} m {} {} l S\n", n(x0), n(y), n(x1), n(y)));
    };
    let mut top = height - MARGIN;
    loop {
        match paper {
            Paper::Single => {
                let bottom = top - 4.0 * SPACE;
                if bottom < MARGIN {
                    break;
                }
                for k in 0..5 {
                    line(top - f64::from(k) * SPACE, &mut ops);
                }
                top = bottom - STAFF_GAP;
            }
            Paper::Tab => {
                let bottom = top - 5.0 * TAB_SPACE;
                if bottom < MARGIN {
                    break;
                }
                for k in 0..6 {
                    line(top - f64::from(k) * TAB_SPACE, &mut ops);
                }
                top = bottom - STAFF_GAP;
            }
            Paper::Grand => {
                let lower_top = top - 4.0 * SPACE - GRAND_INNER;
                let bottom = lower_top - 4.0 * SPACE;
                if bottom < MARGIN {
                    break;
                }
                for k in 0..5 {
                    line(top - f64::from(k) * SPACE, &mut ops);
                    line(lower_top - f64::from(k) * SPACE, &mut ops);
                }
                // The system line joining the two staves at the left.
                ops.push_str(&format!(
                    "1.2 w {} {} m {} {} l S 0.6 w\n",
                    n(x0),
                    n(top),
                    n(x0),
                    n(bottom)
                ));
                top = bottom - GRAND_GAP;
            }
        }
    }
    ops.push_str("Q\n");
    ops
}

/// A new PDF of `pages` pages of `paper`, each `width × height` points, titled `title`.
pub fn manuscript(
    paper: Paper,
    width: f64,
    height: f64,
    pages: usize,
    title: &str,
) -> Result<Vec<u8>, PdfError> {
    if !(1..=MAX_MANUSCRIPT_PAGES).contains(&pages) {
        return Err(PdfError::Invalid(format!(
            "a manuscript has 1 to {MAX_MANUSCRIPT_PAGES} pages"
        )));
    }
    if !(width.is_finite()
        && height.is_finite()
        && (200.0..=2000.0).contains(&width)
        && (200.0..=2000.0).contains(&height))
    {
        return Err(PdfError::Invalid(
            "the page size must be 200 to 2,000 points a side".into(),
        ));
    }
    let fail = |e: &dyn std::fmt::Display| PdfError::Render(format!("making the manuscript: {e}"));
    let mut doc = Cos::new_empty();
    let content = page_content(paper, width, height);
    for i in 0..pages {
        let r = pdfcraft_organize::insert_blank_page(&mut doc, i, width, height)
            .map_err(|e| fail(&e))?;
        let stream = doc.add(Object::Stream(Stream::flate(
            Dict::new(),
            content.as_bytes(),
        )));
        doc.update_dict(r, |d| d.set(b"Contents".to_vec(), Object::Ref(stream)))
            .map_err(|e| fail(&e))?;
    }
    let mut info = Dict::new();
    info.set(b"Title".to_vec(), PdfString::text(title));
    info.set(b"Producer".to_vec(), PdfString::text("ScoreCraft"));
    let info = doc.add(Object::Dict(info));
    doc.trailer_mut().set(b"Info".to_vec(), Object::Ref(info));
    pdfcraft_cos::write_full(&doc, &SaveOptions::default()).map_err(|e| fail(&e))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use std::sync::Arc;

    use super::*;
    use crate::Document;

    #[test]
    fn manuscript_paper_has_its_pages_title_and_lines() {
        for paper in [Paper::Single, Paper::Grand, Paper::Tab] {
            let pdf = manuscript(paper, 612.0, 792.0, 3, "Sketches").unwrap();
            let mut doc = Document::open(Arc::new(pdf)).unwrap();
            assert_eq!(doc.page_count(), 3);
            assert_eq!(doc.metadata().title.as_deref(), Some("Sketches"));
            let bm = doc.render(0, 1.0).unwrap();
            // Some rows are lines (dark across the staff's width), most are paper.
            let dark_rows = (0..bm.height)
                .filter(|y| {
                    let px = bm.pixel(bm.width / 2, *y).unwrap();
                    px[0] < 160
                })
                .count();
            assert!(
                dark_rows > 20 && dark_rows < bm.height as usize / 3,
                "{paper:?} {dark_rows}"
            );
        }
        let lines = |p| page_content(p, 612.0, 792.0).matches(" l S").count();
        assert_eq!(lines(Paper::Single) % 5, 0);
        assert_eq!(lines(Paper::Tab) % 6, 0);
        assert!(manuscript(Paper::Single, 612.0, 792.0, 0, "x").is_err());
        assert!(manuscript(Paper::Single, 5.0, 792.0, 1, "x").is_err());
    }
}
