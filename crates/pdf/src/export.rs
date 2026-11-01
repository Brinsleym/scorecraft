//! Exporting a score as a new PDF with its ink (spec `docs/spec/ink.md`, EXP-2), on PDFCraft's
//! object layer (`pdfcraft-cos`) and page organiser (`pdfcraft-organize`).
//!
//! The imported file is never changed: its pages are copied, in the score's order, into a new
//! document; the score's rotation and crop are applied to the copies; then each page's ink is
//! either **flattened** (drawn into the page content after the original, which is wrapped in
//! `q`/`Q` so its graphics state can't leak) or added as **annotations** (`/Ink`, `/FreeText`,
//! `/Stamp`), each with an appearance stream so every reader draws it the same.
//!
//! Ink arrives as filled polygons in source-page PDF space (the same geometry the CPU renderer
//! draws); this module only writes them.

use std::fmt::Write as _;
use std::sync::Arc;

use pdfcraft_cos::{Dict, Document as Cos, ObjRef, Object, PdfString, SaveOptions, Stream};
use pdfcraft_organize::{BoxSpec, PageBox};

use crate::PdfError;

/// The most pages one export writes.
pub const MAX_EXPORT_PAGES: usize = 20_000;
/// The most polygon points written for one page, so a hostile library can't make a huge file.
pub const MAX_EXPORT_POINTS: usize = 5_000_000;

/// One filled shape, page space, non-zero rule.
#[derive(Debug, Clone, PartialEq)]
pub struct InkFill {
    pub polygons: Vec<Vec<[f64; 2]>>,
    pub rgb: [u8; 3],
    /// 0..=1.
    pub alpha: f64,
    /// Multiplied onto the page (the highlighter).
    pub multiply: bool,
}

/// What a marking is, for its annotation.
#[derive(Debug, Clone, PartialEq)]
pub enum MarkKind {
    /// A pen, highlighter or whiteout stroke, or a hairpin: `/Ink` with these paths.
    Ink {
        paths: Vec<Vec<[f64; 2]>>,
        width: f64,
    },
    /// A text note: `/FreeText`.
    Text { text: String, size: f64 },
    /// A stamp: `/Stamp`.
    Stamp { text: String },
    /// A Score Snip (PAGE-7): `from` (a rectangle of source page `page`) drawn at `to`, as the
    /// source page's own vector content, clipped and scaled. Always drawn into the page.
    Snip {
        page: usize,
        from: [f64; 4],
        to: [f64; 4],
    },
}

/// One marking to export.
#[derive(Debug, Clone, PartialEq)]
pub struct ExportMark {
    pub kind: MarkKind,
    pub fills: Vec<InkFill>,
    /// Its colour, for the annotation's `/C`.
    pub rgb: [u8; 3],
}

/// One page of the exported file.
#[derive(Debug, Clone, PartialEq)]
pub struct ExportPage {
    /// 0-based page of the source file.
    pub file_page: usize,
    /// Degrees clockwise added to the page's own `/Rotate` (a multiple of 90).
    pub rotate: i64,
    /// The score's crop, in source-page PDF space.
    pub crop: Option<[f64; 4]>,
    pub marks: Vec<ExportMark>,
}

/// How ink goes into the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportMode {
    /// Drawn into the page content.
    Flatten,
    /// As annotations with appearance streams.
    Annotations,
}

fn fail(what: &str, e: impl std::fmt::Display) -> PdfError {
    PdfError::Render(format!("{what}: {e}"))
}

/// A number for a content stream: finite, at most three decimals, no trailing zeros.
fn num(v: f64) -> String {
    let v = if v.is_finite() { v } else { 0.0 };
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-0" {
        "0".to_owned()
    } else {
        s.to_owned()
    }
}

fn real(v: f64) -> Object {
    Object::Real(if v.is_finite() { v } else { 0.0 })
}

fn rgb_array(c: [u8; 3]) -> Object {
    Object::Array(c.iter().map(|v| real(f64::from(*v) / 255.0)).collect())
}

/// The graphics states a content stream uses: name, alpha, multiply.
#[derive(Default)]
struct States(Vec<(String, f64, bool)>);

impl States {
    fn name(&mut self, alpha: f64, multiply: bool) -> String {
        let alpha = if alpha.is_finite() {
            alpha.clamp(0.0, 1.0)
        } else {
            1.0
        };
        if let Some((n, _, _)) = self
            .0
            .iter()
            .find(|(_, a, m)| (a - alpha).abs() < 1e-4 && *m == multiply)
        {
            return n.clone();
        }
        let n = format!("SCgs{}", self.0.len());
        self.0.push((n.clone(), alpha, multiply));
        n
    }

    fn dict(&self) -> Dict {
        let mut d = Dict::new();
        for (n, alpha, multiply) in &self.0 {
            let mut g = Dict::new();
            g.set(b"Type".to_vec(), Object::name("ExtGState"));
            g.set(b"ca".to_vec(), real(*alpha));
            g.set(b"CA".to_vec(), real(*alpha));
            g.set(
                b"BM".to_vec(),
                Object::name(if *multiply { "Multiply" } else { "Normal" }),
            );
            d.set(n.as_bytes().to_vec(), Object::Dict(g));
        }
        d
    }
}

/// Content-stream operators that fill `fills`. Points past `budget` are left out.
fn fill_ops(fills: &[InkFill], states: &mut States, budget: &mut usize) -> String {
    let mut out = String::new();
    for f in fills {
        let polys: Vec<&Vec<[f64; 2]>> = f
            .polygons
            .iter()
            .filter(|p| p.len() >= 3 && p.iter().all(|q| q[0].is_finite() && q[1].is_finite()))
            .collect();
        if polys.is_empty() {
            continue;
        }
        let gs = states.name(f.alpha, f.multiply);
        let [r, g, b] = f.rgb.map(|c| f64::from(c) / 255.0);
        let _ = writeln!(out, "q /{gs} gs {} {} {} rg", num(r), num(g), num(b));
        for p in polys {
            if *budget < p.len() {
                break;
            }
            *budget -= p.len();
            for (i, q) in p.iter().enumerate() {
                let op = if i == 0 { "m" } else { "l" };
                let _ = write!(out, "{} {} {op} ", num(q[0]), num(q[1]));
            }
            out.push_str("h\n");
        }
        out.push_str("f Q\n");
    }
    out
}

/// The bounding box of a mark's fills (and paths), grown by `pad`.
fn bounds(m: &ExportMark, pad: f64) -> Option<[f64; 4]> {
    let mut b: Option<[f64; 4]> = None;
    let mut add = |q: &[f64; 2]| {
        if !(q[0].is_finite() && q[1].is_finite()) {
            return;
        }
        b = Some(match b {
            None => [q[0], q[1], q[0], q[1]],
            Some([x0, y0, x1, y1]) => [x0.min(q[0]), y0.min(q[1]), x1.max(q[0]), y1.max(q[1])],
        });
    };
    for f in &m.fills {
        f.polygons.iter().flatten().for_each(&mut add);
    }
    if let MarkKind::Ink { paths, .. } = &m.kind {
        paths.iter().flatten().for_each(&mut add);
    }
    b.map(|[x0, y0, x1, y1]| [x0 - pad, y0 - pad, x1 + pad, y1 + pad])
}

/// A page's resources, resolved and copied, with `states` added to its `/ExtGState`.
fn resources_with(doc: &Cos, page: &Dict, states: &States) -> Dict {
    let mut res = page
        .get(b"Resources")
        .and_then(|o| doc.dict(o))
        .unwrap_or_default();
    let mut gs = res
        .get(b"ExtGState")
        .and_then(|o| doc.dict(o))
        .unwrap_or_default();
    for (k, v) in states.dict().iter() {
        gs.set(k.clone(), v.clone());
    }
    res.set(b"ExtGState".to_vec(), Object::Dict(gs));
    res
}

/// Draws `marks` into the page content.
fn flatten(doc: &mut Cos, page_ref: ObjRef, marks: &[ExportMark]) -> Result<(), PdfError> {
    let mut states = States::default();
    let mut budget = MAX_EXPORT_POINTS;
    let mut ops = String::new();
    for m in marks {
        ops.push_str(&fill_ops(&m.fills, &mut states, &mut budget));
    }
    append_content(doc, page_ref, &ops, &states, &[])
}

/// `[a b c d e f]` matrices: `first` then `then`.
fn mul(first: [f64; 6], then: [f64; 6]) -> [f64; 6] {
    let [a, b, c, d, e, f] = first;
    let [p, q, r, s, t, u] = then;
    [
        a * p + b * r,
        a * q + b * s,
        c * p + d * r,
        c * q + d * s,
        e * p + f * r + t,
        e * q + f * s + u,
    ]
}

fn invert(m: [f64; 6]) -> Option<[f64; 6]> {
    let [a, b, c, d, e, f] = m;
    let det = a * d - b * c;
    if det.abs() < 1e-12 || !det.is_finite() {
        return None;
    }
    let (ia, ib, ic, id) = (d / det, -b / det, -c / det, a / det);
    Some([ia, ib, ic, id, -(e * ia + f * ic), -(e * ib + f * id)])
}

/// Draws the snips among `marks` into the page, each as its source page's content (a form
/// XObject) clipped to its rectangle over white paper, so it covers what is beneath as the
/// reader's copy does, with a thin grey frame.
fn draw_snips(
    doc: &mut Cos,
    src: &Cos,
    page_ref: ObjRef,
    marks: &[ExportMark],
    forms: &mut std::collections::HashMap<usize, ObjRef>,
) -> Result<(), PdfError> {
    let boxes = pdfcraft_organize::page_boxes(src).map_err(|e| fail("reading the pages", e))?;
    let mut ops = String::new();
    let mut xobjects = Vec::new();
    for m in marks {
        let MarkKind::Snip { page, from, to } = &m.kind else {
            continue;
        };
        let (fw, fh) = (from[2] - from[0], from[3] - from[1]);
        let (tw, th) = (to[2] - to[0], to[3] - to[1]);
        if !(fw > 0.0 && fh > 0.0 && tw > 0.0 && th > 0.0) {
            continue;
        }
        let Some(crop) = boxes.get(*page).map(|b| b[1]) else {
            continue;
        };
        let rotate = pdfcraft_organize::page_rotation(src, *page)
            .unwrap_or(0)
            .rem_euclid(360);
        // The form's own /Matrix (see `page_as_form`): undo it, so its content lands in the
        // source page's user space, then map `from` onto `to`.
        let form_m = match rotate {
            90 => [0.0, -1.0, 1.0, 0.0, -crop[1], crop[2]],
            180 => [-1.0, 0.0, 0.0, -1.0, crop[2], crop[3]],
            270 => [0.0, 1.0, -1.0, 0.0, crop[3], -crop[0]],
            _ => [1.0, 0.0, 0.0, 1.0, -crop[0], -crop[1]],
        };
        let Some(undo) = invert(form_m) else {
            continue;
        };
        let (sx, sy) = (tw / fw, th / fh);
        let place = [sx, 0.0, 0.0, sy, to[0] - from[0] * sx, to[1] - from[1] * sy];
        let cm = mul(undo, place);
        let form = match forms.get(page) {
            Some(f) => *f,
            None => {
                let (f, _) = pdfcraft_organize::page_as_form(doc, src, *page)
                    .map_err(|e| fail("copying a snip's page", e))?;
                forms.insert(*page, f);
                f
            }
        };
        let name = format!("SCsnip{}", xobjects.len());
        let _ = writeln!(
            ops,
            "q {} {} {} {} re W n 1 g {} {} {} {} re f {} {} {} {} {} {} cm /{name} Do Q",
            num(to[0]),
            num(to[1]),
            num(tw),
            num(th),
            num(to[0]),
            num(to[1]),
            num(tw),
            num(th),
            num(cm[0]),
            num(cm[1]),
            num(cm[2]),
            num(cm[3]),
            num(cm[4]),
            num(cm[5]),
        );
        let _ = writeln!(
            ops,
            "q 0.627 G 0.5 w {} {} {} {} re S Q",
            num(to[0]),
            num(to[1]),
            num(tw),
            num(th)
        );
        xobjects.push((name, form));
    }
    if ops.is_empty() {
        return Ok(());
    }
    append_content(doc, page_ref, &ops, &States::default(), &xobjects)
}

/// Appends `ops` after the page's content (which is wrapped in `q`/`Q` so its graphics state
/// can't leak), adding `states` and `xobjects` to its resources.
fn append_content(
    doc: &mut Cos,
    page_ref: ObjRef,
    ops: &str,
    states: &States,
    xobjects: &[(String, ObjRef)],
) -> Result<(), PdfError> {
    let mut page = doc
        .dict(&Object::Ref(page_ref))
        .ok_or_else(|| PdfError::Render("a page is missing".into()))?;
    let ops = format!("Q\n{ops}");
    let before = doc.add(Object::Stream(Stream::from_raw(
        Dict::new(),
        b"q\n".to_vec(),
    )));
    let after = doc.add(Object::Stream(Stream::flate(Dict::new(), ops.as_bytes())));
    let mut contents = vec![Object::Ref(before)];
    if let Some(o) = page.get(b"Contents").map(|o| doc.resolve(o)) {
        match &*o {
            Object::Array(a) => contents.extend(a.iter().cloned()),
            Object::Stream(_) => {
                if let Some(Object::Ref(r)) = page.get(b"Contents") {
                    contents.push(Object::Ref(*r));
                }
            }
            _ => {}
        }
    }
    contents.push(Object::Ref(after));
    let mut res = resources_with(doc, &page, states);
    if !xobjects.is_empty() {
        let mut xo = res
            .get(b"XObject")
            .and_then(|o| doc.dict(o))
            .unwrap_or_default();
        for (n, r) in xobjects {
            xo.set(n.as_bytes().to_vec(), Object::Ref(*r));
        }
        res.set(b"XObject".to_vec(), Object::Dict(xo));
    }
    page.set(b"Contents".to_vec(), Object::Array(contents));
    page.set(b"Resources".to_vec(), Object::Dict(res));
    doc.set(page_ref, Object::Dict(page));
    Ok(())
}

/// Adds `marks` to the page as annotations with appearance streams.
fn annotate(
    doc: &mut Cos,
    page_ref: ObjRef,
    marks: &[ExportMark],
    serial: &mut u64,
) -> Result<(), PdfError> {
    let mut page = doc
        .dict(&Object::Ref(page_ref))
        .ok_or_else(|| PdfError::Render("a page is missing".into()))?;
    let mut annots: Vec<Object> = match page.get(b"Annots").map(|o| doc.resolve(o)) {
        Some(o) => o.as_array().cloned().unwrap_or_default(),
        None => Vec::new(),
    };
    let mut budget = MAX_EXPORT_POINTS;
    for m in marks {
        let Some(rect) = bounds(m, 1.0) else {
            continue;
        };
        let mut states = States::default();
        let ops = fill_ops(&m.fills, &mut states, &mut budget);
        let mut res = Dict::new();
        res.set(b"ExtGState".to_vec(), Object::Dict(states.dict()));
        let mut form = Dict::new();
        form.set(b"Type".to_vec(), Object::name("XObject"));
        form.set(b"Subtype".to_vec(), Object::name("Form"));
        form.set(
            b"BBox".to_vec(),
            Object::Array(rect.iter().map(|v| real(*v)).collect()),
        );
        form.set(b"Resources".to_vec(), Object::Dict(res));
        let ap = doc.add(Object::Stream(Stream::flate(form, ops.as_bytes())));
        let mut n = Dict::new();
        n.set(b"N".to_vec(), Object::Ref(ap));
        let mut a = Dict::new();
        a.set(b"Type".to_vec(), Object::name("Annot"));
        a.set(
            b"Rect".to_vec(),
            Object::Array(rect.iter().map(|v| real(*v)).collect()),
        );
        a.set(b"C".to_vec(), rgb_array(m.rgb));
        // Print, so a printed copy shows it too.
        a.set(b"F".to_vec(), Object::Int(4));
        a.set(b"P".to_vec(), Object::Ref(page_ref));
        a.set(b"AP".to_vec(), Object::Dict(n));
        *serial += 1;
        a.set(
            b"NM".to_vec(),
            PdfString::text(&format!("scorecraft-{serial}")),
        );
        a.set(b"T".to_vec(), PdfString::text("ScoreCraft"));
        if let Some(f) = m.fills.first()
            && f.alpha < 1.0
        {
            a.set(b"CA".to_vec(), real(f.alpha));
        }
        match &m.kind {
            MarkKind::Ink { paths, width } => {
                a.set(b"Subtype".to_vec(), Object::name("Ink"));
                let list = paths
                    .iter()
                    .map(|p| {
                        Object::Array(p.iter().flat_map(|q| [real(q[0]), real(q[1])]).collect())
                    })
                    .collect();
                a.set(b"InkList".to_vec(), Object::Array(list));
                let mut bs = Dict::new();
                bs.set(b"W".to_vec(), real(*width));
                a.set(b"BS".to_vec(), Object::Dict(bs));
            }
            MarkKind::Text { text, size } => {
                a.set(b"Subtype".to_vec(), Object::name("FreeText"));
                a.set(b"Contents".to_vec(), PdfString::text(text));
                let [r, g, b] = m.rgb.map(|c| f64::from(c) / 255.0);
                a.set(
                    b"DA".to_vec(),
                    PdfString::text(&format!(
                        "/Helv {} Tf {} {} {} rg",
                        num(*size),
                        num(r),
                        num(g),
                        num(b)
                    )),
                );
            }
            MarkKind::Stamp { text } => {
                a.set(b"Subtype".to_vec(), Object::name("Stamp"));
                a.set(b"Name".to_vec(), Object::name("ScoreCraftDynamic"));
                a.set(b"Contents".to_vec(), PdfString::text(text));
            }
            // Drawn into the page by `draw_snips`, never an annotation.
            MarkKind::Snip { .. } => continue,
        }
        annots.push(Object::Ref(doc.add(Object::Dict(a))));
    }
    page.set(b"Annots".to_vec(), Object::Array(annots));
    doc.set(page_ref, Object::Dict(page));
    Ok(())
}

/// A new PDF of `pages` from the source file `bytes`, with their ink written as `mode` says.
pub fn export_pdf(
    bytes: Arc<Vec<u8>>,
    password: Option<&str>,
    pages: &[ExportPage],
    mode: ExportMode,
) -> Result<Vec<u8>, PdfError> {
    if pages.is_empty() {
        return Err(PdfError::Invalid("there are no pages to export".into()));
    }
    if pages.len() > MAX_EXPORT_PAGES {
        return Err(PdfError::Unsupported(format!(
            "exporting more than {MAX_EXPORT_PAGES} pages"
        )));
    }
    let src = Cos::open_with_password(bytes, password).map_err(|e| fail("reading the score", e))?;
    let count = pdfcraft_organize::page_count(&src).map_err(|e| fail("reading the pages", e))?;
    if let Some(p) = pages.iter().find(|p| p.file_page >= count) {
        return Err(PdfError::PageOutOfRange {
            page: p.file_page,
            count,
        });
    }
    let mut dst = Cos::new_empty();
    let order: Vec<usize> = pages.iter().map(|p| p.file_page).collect();
    let refs = pdfcraft_organize::import_pages(&mut dst, &src, &order, 0)
        .map_err(|e| fail("copying the pages", e))?;
    let mut serial = 0;
    let mut forms = std::collections::HashMap::new();
    for (i, (page, r)) in pages.iter().zip(&refs).enumerate() {
        if page.rotate.rem_euclid(360) != 0 {
            let turn = page.rotate.rem_euclid(360) / 90 * 90;
            pdfcraft_organize::rotate_pages(&mut dst, &[i], turn)
                .map_err(|e| fail("turning a page", e))?;
        }
        if let Some(c) = page.crop
            && c.iter().all(|v| v.is_finite())
            && c[2] > c[0]
            && c[3] > c[1]
        {
            pdfcraft_organize::set_page_box(&mut dst, &[i], PageBox::Crop, BoxSpec::Rect(c))
                .map_err(|e| fail("cropping a page", e))?;
        }
        if page.marks.is_empty() {
            continue;
        }
        // Snips are page content in both modes; the rest go as the mode says.
        draw_snips(&mut dst, &src, *r, &page.marks, &mut forms)?;
        let rest: Vec<ExportMark> = page
            .marks
            .iter()
            .filter(|m| !matches!(m.kind, MarkKind::Snip { .. }))
            .cloned()
            .collect();
        if rest.is_empty() {
            continue;
        }
        match mode {
            ExportMode::Flatten => flatten(&mut dst, *r, &rest)?,
            ExportMode::Annotations => annotate(&mut dst, *r, &rest, &mut serial)?,
        }
    }
    pdfcraft_cos::write_full(&dst, &SaveOptions::default()).map_err(|e| fail("writing the PDF", e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Document;

    fn square(x: f64, y: f64, s: f64, rgb: [u8; 3], multiply: bool) -> InkFill {
        InkFill {
            polygons: vec![vec![[x, y], [x + s, y], [x + s, y + s], [x, y + s]]],
            rgb,
            alpha: 1.0,
            multiply,
        }
    }

    fn mark(fill: InkFill) -> ExportMark {
        ExportMark {
            rgb: fill.rgb,
            kind: MarkKind::Ink {
                paths: vec![vec![[0.0, 0.0], [1.0, 1.0]]],
                width: 2.0,
            },
            fills: vec![fill],
        }
    }

    fn source(pages: usize) -> Arc<Vec<u8>> {
        Arc::new(
            scorecraft_testkit::pdf::PdfBuilder::new()
                .pages(pages, scorecraft_testkit::pdf::PageSpec::default())
                .build(),
        )
    }

    /// The colour of the pixel at page point (`x`, `y`) of page `i` rendered at 1×.
    fn pixel(doc: &mut Document, i: usize, x: f64, y: f64) -> [u8; 3] {
        let bm = doc.render(i, 1.0).unwrap();
        let g = doc.page(i).unwrap();
        let h = f64::from(g.crop[3] - g.crop[1]);
        let (px, py) = (x as u32, (h - y) as u32);
        let w = bm.words[(py * bm.width + px) as usize].to_ne_bytes();
        [w[0], w[1], w[2]]
    }

    #[test]
    fn flattened_ink_is_drawn_in_order_and_the_source_is_untouched() {
        let bytes = source(3);
        let before = bytes.clone();
        let pages = vec![
            ExportPage {
                file_page: 2,
                rotate: 0,
                crop: None,
                marks: vec![mark(square(100.0, 100.0, 50.0, [200, 0, 0], false))],
            },
            ExportPage {
                file_page: 0,
                rotate: 90,
                crop: None,
                marks: Vec::new(),
            },
        ];
        let out = export_pdf(bytes, None, &pages, ExportMode::Flatten).unwrap();
        assert_eq!(*before, *source(3), "the source bytes are unchanged");
        let mut doc = Document::open(Arc::new(out)).unwrap();
        assert_eq!(doc.page_count(), 2);
        assert_eq!(doc.page(1).unwrap().rotation % 360, 90);
        let inside = pixel(&mut doc, 0, 125.0, 125.0);
        assert!(inside[0] > 150 && inside[1] < 60, "{inside:?}");
        let outside = pixel(&mut doc, 0, 300.0, 300.0);
        assert!(outside.iter().all(|c| *c > 200), "{outside:?}");
    }

    #[test]
    fn annotations_carry_appearances_and_crops_apply() {
        let pages = vec![ExportPage {
            file_page: 0,
            rotate: 0,
            crop: Some([50.0, 50.0, 500.0, 700.0]),
            marks: vec![
                mark(square(100.0, 100.0, 50.0, [0, 0, 200], false)),
                ExportMark {
                    rgb: [0, 0, 0],
                    kind: MarkKind::Text {
                        text: "rit.".into(),
                        size: 12.0,
                    },
                    fills: vec![square(200.0, 200.0, 10.0, [0, 0, 0], false)],
                },
            ],
        }];
        let out = export_pdf(source(1), None, &pages, ExportMode::Annotations).unwrap();
        let cos = Cos::open(Arc::new(out.clone())).unwrap();
        let refs = pdfcraft_organize::pages(&cos).unwrap();
        let page = cos.dict(&Object::Ref(refs[0].obj)).unwrap();
        let annots = cos.resolve(page.get(b"Annots").unwrap());
        assert_eq!(annots.as_array().unwrap().len(), 2);
        let doc = Document::open(Arc::new(out)).unwrap();
        let g = doc.page(0).unwrap();
        assert_eq!(g.crop, [50.0, 50.0, 500.0, 700.0]);
    }

    #[test]
    fn snips_copy_the_source_pages_content() {
        // Page 1 gets a red square; page 2's snip of that square lands at the same place.
        let pages = vec![
            ExportPage {
                file_page: 0,
                rotate: 0,
                crop: None,
                marks: vec![mark(square(100.0, 100.0, 50.0, [200, 0, 0], false))],
            },
            ExportPage {
                file_page: 1,
                rotate: 0,
                crop: None,
                marks: vec![ExportMark {
                    rgb: [0, 0, 0],
                    kind: MarkKind::Snip {
                        page: 0,
                        from: [0.0, 0.0, 612.0, 792.0],
                        to: [0.0, 0.0, 612.0, 792.0],
                    },
                    fills: Vec::new(),
                }],
            },
        ];
        let out = export_pdf(source(2), None, &pages, ExportMode::Annotations).unwrap();
        let mut doc = Document::open(Arc::new(out)).unwrap();
        // The snip copies the source page's own content (not its ink, which is drawn on top
        // of page 1 alone): page 2 at the square's place looks like page 1 without ink.
        let mut original = Document::open(source(2)).unwrap();
        for (x, y) in [(125.0, 125.0), (300.0, 300.0), (72.0, 700.0)] {
            assert_eq!(
                pixel(&mut doc, 1, x, y),
                pixel(&mut original, 0, x, y),
                "at {x}, {y}"
            );
        }
        // The frame is drawn.
        let frame = pixel(&mut doc, 1, 0.2, 400.0);
        assert!(frame.iter().all(|c| *c < 240), "{frame:?}");
    }

    #[test]
    fn bad_requests_are_refused() {
        assert!(export_pdf(source(1), None, &[], ExportMode::Flatten).is_err());
        let p = ExportPage {
            file_page: 5,
            rotate: 0,
            crop: None,
            marks: Vec::new(),
        };
        assert!(matches!(
            export_pdf(source(1), None, &[p], ExportMode::Flatten),
            Err(PdfError::PageOutOfRange { .. })
        ));
        assert!(export_pdf(Arc::new(b"nope".to_vec()), None, &[], ExportMode::Flatten).is_err());
        assert_eq!(num(1.5), "1.5");
        assert_eq!(num(2.0), "2");
        assert_eq!(num(f64::NAN), "0");
        assert_eq!(num(-0.0001), "0");
    }
}
