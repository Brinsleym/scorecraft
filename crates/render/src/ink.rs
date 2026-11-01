//! Ink on a rendered page, on the CPU (architecture §7; spec `docs/spec/ink.md`).
//!
//! Each stroke's outline (from `scorecraft-ink`) is filled into a coverage mask with the
//! non-zero rule, so overlapping parts of one stroke cover a pixel once, then blended onto the
//! page bitmap:
//!
//! - **pen**: normal "over" blending in the stroke's colour;
//! - **highlighter**: multiply, so notation under it stays black and the paper takes the colour
//!   (on a dark page, where multiply would hide it, it blends "over" instead);
//! - **whiteout**: the page's paper colour, opaque.
//!
//! This is the oracle every other ink renderer is checked against, and what export and
//! `view.snapshot` draw with.

use scorecraft_doc::{
    Annotation, AnnotationKind, Color, Hairpin, Layer, Stamp, Stroke, TextNote, Tool,
};
use scorecraft_geom::{Affine, PageBox, Point, Rect, Vec2};
use scorecraft_ink::{PressureCurve, outline};
use scorecraft_pdf::{Bitmap, PageGeometry};
use scorecraft_text::{Face, FaceKind, LINE_SPACING, layout};

use crate::Filter;

/// The highlighter's strength laid over a dark page.
const DARK_HIGHLIGHT: f32 = 0.4;

/// The ink a page carries, as the page cache keeps it: the markings to draw in drawing order
/// and a key that changes whenever they do (the engine fingerprints ids and stamps).
#[derive(Debug, Clone)]
pub struct PageInk {
    pub key: u64,
    pub marks: std::sync::Arc<Vec<AnnotationKind>>,
    pub curve: PressureCurve,
}

/// Whether a renderer draws this kind of marking into the page. Snips are (PAGE-7: the copied
/// bars are part of the page's picture); shortcuts are not (the reader outlines them in markup
/// mode only).
pub fn is_drawn(kind: &AnnotationKind) -> bool {
    matches!(
        kind,
        AnnotationKind::Stroke(_)
            | AnnotationKind::Text(_)
            | AnnotationKind::Stamp(_)
            | AnnotationKind::Hairpin(_)
            | AnnotationKind::Snip(_)
    )
}

/// Draws a Score Snip (PAGE-7) onto `bm`: every pixel of the snip's place takes the matching
/// pixel of `src` (its source page rendered at the same scale), and a thin grey frame marks it.
/// `m` maps the destination page to `bm`, `src_m` the source page to `src`.
pub fn draw_snip(
    bm: &mut Bitmap,
    m: Affine,
    src: &Bitmap,
    src_m: Affine,
    snip: &scorecraft_doc::Snip,
) {
    let (from, to) = (snip.from.rect, snip.to.rect);
    if !(from.is_finite() && to.is_finite() && to.width() > 0.0 && to.height() > 0.0) {
        return;
    }
    let Some(inv) = m.inverse() else {
        return;
    };
    let px = m.apply_rect(to);
    let clamp = |v: f64, hi: u32| v.floor().clamp(0.0, f64::from(hi)) as u32;
    let (x0, x1) = (clamp(px.x0, bm.width), clamp(px.x1.ceil(), bm.width));
    let (y0, y1) = (clamp(px.y0, bm.height), clamp(px.y1.ceil(), bm.height));
    let frame = [160u8, 160, 160, 255];
    for y in y0..y1 {
        for x in x0..x1 {
            let p = inv.apply(Point::new(f64::from(x) + 0.5, f64::from(y) + 0.5));
            let (u, v) = ((p.x - to.x0) / to.width(), (p.y - to.y0) / to.height());
            if !(0.0..=1.0).contains(&u) || !(0.0..=1.0).contains(&v) {
                continue;
            }
            let edge = x == x0 || y == y0 || x + 1 == x1 || y + 1 == y1;
            let word = if edge {
                u32::from_ne_bytes(frame)
            } else {
                let q = src_m.apply(Point::new(
                    from.x0 + u * from.width(),
                    from.y0 + v * from.height(),
                ));
                let (sx, sy) = (q.x.floor(), q.y.floor());
                if sx < 0.0 || sy < 0.0 || sx >= f64::from(src.width) || sy >= f64::from(src.height)
                {
                    continue;
                }
                match src
                    .words
                    .get((sy as usize) * (src.width as usize) + sx as usize)
                {
                    Some(w) => *w,
                    None => continue,
                }
            };
            if let Some(dst) = bm
                .words
                .get_mut((y as usize) * (bm.width as usize) + x as usize)
            {
                *dst = word;
            }
        }
    }
}

/// The markings to draw on a page, in drawing order: layers bottom first in list order, each
/// layer's markings in the order they were made. Hidden layers, markings on a layer the score
/// doesn't have, and kinds that aren't drawn into the page are left out.
pub fn visible_marks<'a>(
    layers: &[Layer],
    annotations: &'a [Annotation],
) -> Vec<&'a AnnotationKind> {
    let mut out = Vec::new();
    for layer in layers.iter().filter(|l| l.visible) {
        for a in annotations.iter().filter(|a| a.layer == layer.id) {
            if is_drawn(&a.kind) {
                out.push(&a.kind);
            }
        }
    }
    out
}

/// A file page's geometry as a [`PageBox`]: its crop box (in unrotated PDF space) and `/Rotate`.
pub fn page_box(geometry: &PageGeometry) -> PageBox {
    let [x0, y0, x1, y1] = geometry.crop.map(f64::from);
    let rotation = match geometry.rotation % 360 {
        90 => scorecraft_geom::Rotation::R90,
        180 => scorecraft_geom::Rotation::R180,
        270 => scorecraft_geom::Rotation::R270,
        _ => scorecraft_geom::Rotation::R0,
    };
    PageBox::new(Rect::new(x0, y0, x1, y1)).with_rotation(rotation)
}

/// The transform from source-page PDF space to `bitmap`'s pixels, for a bitmap of the whole page
/// `geometry` describes (crop box, `/Rotate`). The bitmap's own size sets the scale, so a render
/// whose scale was capped or rounded still lines up.
pub fn page_to_bitmap(geometry: &PageGeometry, bitmap: &Bitmap) -> Option<Affine> {
    let page = page_box(geometry);
    let size = page.display_size_pt();
    if !(size.width > 0.0 && size.height > 0.0) {
        return None;
    }
    let sx = f64::from(bitmap.width) / size.width;
    let sy = f64::from(bitmap.height) / size.height;
    let m = page.to_display(1.0).then(Affine::scale_xy(sx, sy));
    m.is_finite().then_some(m)
}

/// `base` with `marks` drawn on it in order. Returns `None` when there is nothing to draw (the
/// caller keeps the original bitmap, no copy) or the transform is degenerate.
pub fn compose(
    base: &Bitmap,
    to_bitmap: Affine,
    marks: &[&AnnotationKind],
    filter: Filter,
    curve: &PressureCurve,
) -> Option<Bitmap> {
    if !marks.iter().any(|m| is_drawn(m)) || !to_bitmap.is_finite() {
        return None;
    }
    let mut out = base.clone();
    for mark in marks {
        match mark {
            AnnotationKind::Stroke(s) => draw_stroke(&mut out, to_bitmap, s, filter, curve),
            AnnotationKind::Text(t) => draw_text(&mut out, to_bitmap, t, filter),
            AnnotationKind::Stamp(st) => draw_stamp(&mut out, to_bitmap, st, filter),
            AnnotationKind::Hairpin(h) => draw_hairpin(&mut out, to_bitmap, h, filter, curve),
            _ => {}
        }
    }
    Some(out)
}

/// One filled shape of a marking, in source-page PDF space: what every renderer draws, the CPU
/// compositor here and the PDF export (EXP-2). Polygons are filled with the non-zero rule.
#[derive(Debug, Clone, PartialEq)]
pub struct Fill {
    pub polygons: Vec<Vec<Point>>,
    /// The colour, alpha ignored (it is in `alpha`).
    pub color: Color,
    /// 0..=1.
    pub alpha: f32,
    /// Multiplied onto the page (the highlighter) rather than laid over it.
    pub multiply: bool,
}

/// The fills that draw `mark` under `filter`, in order. Empty for marks that draw nothing.
pub fn fills(mark: &AnnotationKind, filter: Filter, curve: &PressureCurve) -> Vec<Fill> {
    match mark {
        AnnotationKind::Stroke(s) => stroke_fill(s, filter, curve).into_iter().collect(),
        AnnotationKind::Text(t) => text_fills(t, filter),
        AnnotationKind::Stamp(st) => stamp_fill(st, filter).into_iter().collect(),
        AnnotationKind::Hairpin(h) => hairpin_fills(h, filter, curve),
        _ => Vec::new(),
    }
}

/// A text note (spec ANNO-8): its optional background, then its lines, wrapped to the note's
/// width and set from its top-left corner.
fn text_fills(note: &TextNote, filter: Filter) -> Vec<Fill> {
    let mut out = Vec::new();
    let style = &note.style;
    let size = f64::from(style.size);
    let Some(face) = Face::get(FaceKind::Note, style.bold, style.italic) else {
        return out;
    };
    if !(size.is_finite() && size > 0.0 && note.rect.is_finite()) {
        return out;
    }
    if let Some(bg) = style.background {
        out.push(Fill {
            polygons: vec![note.rect.corners().to_vec()],
            color: paper_mapped(bg, filter),
            alpha: 1.0,
            multiply: false,
        });
    }
    // A hair wider than the box, so rounding never re-wraps a line that fitted.
    let laid = layout(&face, &note.text, size, note.rect.width() + 0.01);
    let color = ink_color(Tool::Pen, style.color, filter);
    let mut polys = Vec::new();
    for (i, line) in laid.lines.iter().enumerate() {
        let baseline = note.rect.y1 - laid.baseline - i as f64 * size * LINE_SPACING;
        polys.extend(face.outlines(line, size, Point::new(note.rect.x0, baseline)));
    }
    out.push(Fill {
        polygons: polys,
        color,
        alpha: f32::from(color.a) / 255.0,
        multiply: false,
    });
    out
}

/// A dynamic stamp (spec ANNO-9): its letters in the dynamics face, centred in its box. SMuFL
/// glyph stamps need a music font and draw nothing until one is built in.
fn stamp_fill(stamp: &Stamp, filter: Filter) -> Option<Fill> {
    if stamp.text.is_empty() || !stamp.rect.is_finite() {
        return None;
    }
    let size = if stamp.size.is_finite() && stamp.size > 0.0 {
        f64::from(stamp.size)
    } else {
        stamp.rect.height()
    };
    let face = Face::get(FaceKind::Dynamic, false, false)?;
    let (asc, desc) = face.ascent_descent(size);
    let c = stamp.rect.center();
    let origin = Point::new(
        c.x - face.advance(&stamp.text, size) / 2.0,
        c.y - (asc - desc) / 2.0,
    );
    let color = ink_color(Tool::Pen, stamp.color, filter);
    Some(Fill {
        polygons: face.outlines(&stamp.text, size, origin),
        color,
        alpha: f32::from(color.a) / 255.0,
        multiply: false,
    })
}

/// Draws a text note (spec ANNO-8).
pub fn draw_text(bm: &mut Bitmap, m: Affine, note: &TextNote, filter: Filter) {
    for f in text_fills(note, filter) {
        paint(bm, m, &f);
    }
}

/// Draws a dynamic stamp (spec ANNO-9).
pub fn draw_stamp(bm: &mut Bitmap, m: Affine, stamp: &Stamp, filter: Filter) {
    if let Some(f) = stamp_fill(stamp, filter) {
        paint(bm, m, &f);
    }
}

/// The two lines of a hairpin, each from its open end to its point (spec ANNO-9).
pub fn hairpin_lines(h: &Hairpin) -> Option<[[Point; 2]; 2]> {
    let d = h.to - h.from;
    let len = d.length();
    if !(len > 0.0 && len.is_finite() && h.opening.is_finite()) {
        return None;
    }
    let n = Vec2::new(-d.y / len, d.x / len) * (f64::from(h.opening.max(0.0)) / 2.0);
    let (tip, open) = if h.crescendo {
        (h.from, h.to)
    } else {
        (h.to, h.from)
    };
    Some([[open + n, tip], [open - n, tip]])
}

/// A hairpin's two lines as round-ended strokes.
pub fn hairpin_strokes(h: &Hairpin) -> Vec<Stroke> {
    let Some(lines) = hairpin_lines(h) else {
        return Vec::new();
    };
    let width = if h.width.is_finite() && h.width > 0.0 {
        h.width
    } else {
        1.0
    };
    lines
        .into_iter()
        .map(|[a, b]| Stroke {
            tool: Tool::Pen,
            color: h.color,
            width,
            nib: scorecraft_doc::Nib::Round,
            points: vec![
                scorecraft_doc::InkPoint::new(a.x as f32, a.y as f32),
                scorecraft_doc::InkPoint::new(b.x as f32, b.y as f32),
            ],
        })
        .collect()
}

fn hairpin_fills(h: &Hairpin, filter: Filter, curve: &PressureCurve) -> Vec<Fill> {
    hairpin_strokes(h)
        .iter()
        .filter_map(|s| stroke_fill(s, filter, curve))
        .collect()
}

/// Draws a hairpin as two round-ended lines.
pub fn draw_hairpin(
    bm: &mut Bitmap,
    m: Affine,
    h: &Hairpin,
    filter: Filter,
    curve: &PressureCurve,
) {
    for f in hairpin_fills(h, filter, curve) {
        paint(bm, m, &f);
    }
}

/// A background colour under `filter`: white becomes the filter's paper, like whiteout.
fn paper_mapped(c: Color, filter: Filter) -> Color {
    if Color::rgb(c.r, c.g, c.b) == Color::WHITE {
        let mut p = ink_color(Tool::Whiteout, c, filter);
        p.a = c.a;
        p
    } else {
        c
    }
}

/// Paints one fill onto `bm`.
fn paint(bm: &mut Bitmap, m: Affine, f: &Fill) {
    let Some(mask) = Mask::fill(&f.polygons, m, bm.width, bm.height) else {
        return;
    };
    blend(bm, &mask, f.color, f.alpha, f.multiply);
}

/// A stroke's fill under `filter`.
fn stroke_fill(s: &Stroke, filter: Filter, curve: &PressureCurve) -> Option<Fill> {
    let o = outline(s, curve);
    if o.polygons.is_empty() {
        return None;
    }
    let color = ink_color(s.tool, s.color, filter);
    let mut alpha = f32::from(color.a) / 255.0;
    if s.tool == Tool::Highlighter && filter == Filter::Dark {
        // Multiplying a dark page would hide the highlighter; lay it over at 40% instead, so the
        // light notation still reads through it.
        alpha = alpha.min(DARK_HIGHLIGHT);
    }
    Some(Fill {
        polygons: o.polygons,
        color,
        alpha,
        multiply: s.tool == Tool::Highlighter && filter != Filter::Dark,
    })
}

/// Draws one stroke onto `bm`.
pub fn draw_stroke(bm: &mut Bitmap, m: Affine, s: &Stroke, filter: Filter, curve: &PressureCurve) {
    if let Some(f) = stroke_fill(s, filter, curve) {
        paint(bm, m, &f);
    }
}

/// Blends `color` at `alpha` through `mask` onto `bm`: multiplied (the highlighter) or "over".
fn blend(bm: &mut Bitmap, mask: &Mask, color: Color, alpha: f32, multiply: bool) {
    let src = [color.r, color.g, color.b].map(|c| f32::from(c) / 255.0);
    for row in 0..mask.h {
        let y = mask.y0 + row;
        for col in 0..mask.w {
            let cov = mask.at(col, row);
            if cov <= 0.0 {
                continue;
            }
            let x = mask.x0 + col;
            let Some(word) = y
                .checked_mul(bm.width)
                .and_then(|i| i.checked_add(x))
                .and_then(|i| bm.words.get_mut(i as usize))
            else {
                continue;
            };
            let [dr, dg, db, da] = word.to_ne_bytes().map(f32::from);
            let [sr, sg, sb] = src;
            let k = alpha * cov;
            let px = if multiply {
                let mul = |d: f32, s: f32| to_byte(d * (1.0 - k * (1.0 - s)));
                [mul(dr, sr), mul(dg, sg), mul(db, sb), to_byte(da)]
            } else {
                // Premultiplied "over".
                let over = |d: f32, s: f32| to_byte(s * 255.0 * k + d * (1.0 - k));
                [
                    over(dr, sr),
                    over(dg, sg),
                    over(db, sb),
                    to_byte(255.0 * k + da * (1.0 - k)),
                ]
            };
            *word = u32::from_ne_bytes(px);
        }
    }
}

fn to_byte(v: f32) -> u8 {
    v.round().clamp(0.0, 255.0) as u8
}

/// The colour a stroke draws in under `filter` (spec, "Drawing and colours under filters").
pub fn ink_color(tool: Tool, color: Color, filter: Filter) -> Color {
    match tool {
        Tool::Whiteout => {
            let paper = filter.colors().map_or([255, 255, 255], |c| c.paper);
            Color::rgb(paper[0], paper[1], paper[2])
        }
        Tool::Pen if filter == Filter::Dark => {
            let luma = 0.2126 * f32::from(color.r)
                + 0.7152 * f32::from(color.g)
                + 0.0722 * f32::from(color.b);
            if luma < 128.0 {
                let [r, g, b] = invert_lightness([color.r, color.g, color.b]);
                Color::rgba(r, g, b, color.a)
            } else {
                color
            }
        }
        _ => color,
    }
}

/// The colour with its HSL lightness inverted and its hue and saturation kept.
fn invert_lightness(rgb: [u8; 3]) -> [u8; 3] {
    let [r, g, b] = rgb.map(|c| f32::from(c) / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    let d = max - min;
    let l2 = 1.0 - l;
    if d <= f32::EPSILON {
        let v = to_byte(l2 * 255.0);
        return [v, v, v];
    }
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };
    let h = if max == r {
        ((g - b) / d).rem_euclid(6.0)
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    } / 6.0;
    let q = if l2 < 0.5 {
        l2 * (1.0 + s)
    } else {
        l2 + s - l2 * s
    };
    let p = 2.0 * l2 - q;
    let ch = |t: f32| {
        let t = t.rem_euclid(1.0);
        let v = if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        };
        to_byte(v * 255.0)
    };
    [ch(h + 1.0 / 3.0), ch(h), ch(h - 1.0 / 3.0)]
}

/// Coverage (0..=1) of polygons over a pixel box, by signed-area accumulation: each edge adds
/// the area it sweeps to the cells it crosses, and a running sum along each row gives the
/// winding-weighted coverage. Clamping the sum's magnitude to 1 is the non-zero union.
struct Mask {
    x0: u32,
    y0: u32,
    w: u32,
    h: u32,
    /// `w + 2` accumulators per row (edges right of the box land in the spare cells).
    acc: Vec<f32>,
}

/// Largest mask, in pixels: a stroke covering more is drawn over this much of the page only.
const MAX_MASK_PIXELS: u64 = 64 * 1024 * 1024;

impl Mask {
    fn fill(polys: &[Vec<Point>], m: Affine, width: u32, height: u32) -> Option<Mask> {
        let mapped: Vec<Vec<Point>> = polys
            .iter()
            .map(|p| p.iter().map(|q| m.apply(*q)).collect::<Vec<_>>())
            .filter(|p: &Vec<Point>| p.iter().all(|q| q.is_finite()))
            .collect();
        let b = Rect::bounding(mapped.iter().flatten().copied())?;
        let x0 = b.x0.floor().max(0.0);
        let y0 = b.y0.floor().max(0.0);
        let x1 = b.x1.ceil().min(f64::from(width));
        let y1 = b.y1.ceil().min(f64::from(height));
        if x1 <= x0 || y1 <= y0 {
            return None;
        }
        let (x0, y0) = (x0 as u32, y0 as u32);
        let (w, h) = (
            (x1 as u32).saturating_sub(x0),
            (y1 as u32).saturating_sub(y0),
        );
        if u64::from(w) * u64::from(h) > MAX_MASK_PIXELS || w == 0 || h == 0 {
            return None;
        }
        let stride = w as usize + 2;
        let mut mask = Mask {
            x0,
            y0,
            w,
            h,
            acc: vec![0.0; stride * h as usize],
        };
        let off = Point::new(f64::from(x0), f64::from(y0));
        for poly in &mapped {
            for (i, a) in poly.iter().enumerate() {
                let b = poly.get((i + 1) % poly.len()).unwrap_or(a);
                mask.line(
                    Point::new(a.x - off.x, a.y - off.y),
                    Point::new(b.x - off.x, b.y - off.y),
                );
            }
        }
        // Running sums turn edge contributions into coverage.
        for row in 0..h as usize {
            let mut sum = 0.0f32;
            for col in 0..stride {
                if let Some(c) = mask.acc.get_mut(row * stride + col) {
                    sum += *c;
                    *c = sum.abs().min(1.0);
                }
            }
        }
        Some(mask)
    }

    fn at(&self, col: u32, row: u32) -> f32 {
        let stride = self.w as usize + 2;
        self.acc
            .get(row as usize * stride + col as usize)
            .copied()
            .unwrap_or(0.0)
    }

    fn add(&mut self, row: usize, col: i64, v: f32) {
        let stride = self.w as usize + 2;
        let col = col.clamp(0, stride as i64 - 1) as usize;
        if let Some(c) = self.acc.get_mut(row * stride + col) {
            *c += v;
        }
    }

    /// Accumulates one edge, in mask pixels (x clamped into the box; rows outside skipped).
    fn line(&mut self, p0: Point, p1: Point) {
        if p0.y == p1.y {
            return;
        }
        let (dir, a, b) = if p0.y < p1.y {
            (1.0f32, p0, p1)
        } else {
            (-1.0f32, p1, p0)
        };
        let h = f64::from(self.h);
        let wmax = f64::from(self.w) + 1.0;
        let dxdy = (b.x - a.x) / (b.y - a.y);
        let y_start = a.y.max(0.0);
        let y_end = b.y.min(h);
        if y_start >= y_end {
            return;
        }
        let mut x = a.x + dxdy * (y_start - a.y);
        let mut y = y_start;
        while y < y_end {
            let row = y.floor();
            let next = (row + 1.0).min(y_end);
            let dy = next - y;
            let xnext = x + dxdy * dy;
            let d = (dy as f32) * dir;
            let (xa, xb) = if x < xnext { (x, xnext) } else { (xnext, x) };
            let (xa, xb) = (xa.clamp(0.0, wmax), xb.clamp(0.0, wmax));
            let r = row as usize;
            let x0f = xa.floor();
            let x1c = xb.ceil();
            let (x0i, x1i) = (x0f as i64, x1c as i64);
            if x1i <= x0i + 1 {
                // The edge stays within one cell on this row.
                let xm = ((xa + xb) * 0.5 - x0f) as f32;
                self.add(r, x0i, d * (1.0 - xm));
                self.add(r, x0i + 1, d * xm);
            } else {
                let s = (1.0 / (xb - xa)) as f32;
                let x0frac = (xa - x0f) as f32;
                let a0 = 0.5 * s * (1.0 - x0frac) * (1.0 - x0frac);
                let x1frac = (xb - x1c + 1.0) as f32;
                let am = 0.5 * s * x1frac * x1frac;
                self.add(r, x0i, d * a0);
                if x1i == x0i + 2 {
                    self.add(r, x0i + 1, d * (1.0 - a0 - am));
                } else {
                    let a1 = s * (1.5 - x0frac);
                    self.add(r, x0i + 1, d * (a1 - a0));
                    for xi in x0i + 2..x1i - 1 {
                        self.add(r, xi, d * s);
                    }
                    let a2 = a1 + (x1i - x0i - 3) as f32 * s;
                    self.add(r, x1i - 1, d * (1.0 - a2 - am));
                }
                self.add(r, x1i, d * am);
            }
            x = xnext;
            y = next;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scorecraft_doc::InkPoint;

    fn page(w: u32, h: u32, rgba: [u8; 4]) -> Bitmap {
        Bitmap {
            width: w,
            height: h,
            words: vec![u32::from_ne_bytes(rgba); (w * h) as usize],
        }
    }

    fn px(bm: &Bitmap, x: u32, y: u32) -> [u8; 4] {
        bm.pixel(x, y).unwrap()
    }

    fn stroke(tool: Tool, color: Color, width: f32, pts: &[(f32, f32)]) -> Stroke {
        Stroke {
            tool,
            color,
            width,
            nib: Default::default(),
            points: pts.iter().map(|&(x, y)| InkPoint::new(x, y)).collect(),
        }
    }

    /// 1 pixel per point, y flipped: a 100 × 100 pt page.
    fn flip() -> Affine {
        Affine::new([1.0, 0.0, 0.0, -1.0, 0.0, 100.0])
    }

    const WHITE: [u8; 4] = [255, 255, 255, 255];

    #[test]
    fn square_coverage_is_exact() {
        let sq = vec![vec![
            Point::new(10.0, 10.0),
            Point::new(20.0, 10.0),
            Point::new(20.0, 20.0),
            Point::new(10.0, 20.0),
        ]];
        let m = Mask::fill(&sq, Affine::IDENTITY, 50, 50).unwrap();
        let total: f32 = (0..m.h)
            .flat_map(|r| (0..m.w).map(move |c| (c, r)))
            .map(|(c, r)| m.at(c, r))
            .sum();
        assert!((total - 100.0).abs() < 0.01, "{total}");
        // Half-pixel edges give half coverage.
        let half = vec![vec![
            Point::new(10.5, 10.0),
            Point::new(20.0, 10.0),
            Point::new(20.0, 20.0),
            Point::new(10.5, 20.0),
        ]];
        let m = Mask::fill(&half, Affine::IDENTITY, 50, 50).unwrap();
        assert!((m.at(0, 5) - 0.5).abs() < 1e-4);
        assert!((m.at(5, 5) - 1.0).abs() < 1e-4);
    }

    #[test]
    fn pen_draws_its_colour() {
        let mut bm = page(100, 100, WHITE);
        let s = stroke(Tool::Pen, Color::RED, 4.0, &[(10.0, 50.0), (90.0, 50.0)]);
        draw_stroke(
            &mut bm,
            flip(),
            &s,
            Filter::Normal,
            &PressureCurve::default(),
        );
        assert_eq!(px(&bm, 50, 49), [220, 30, 30, 255]);
        assert_eq!(px(&bm, 50, 10), WHITE);
    }

    #[test]
    fn ink_5_highlighter_does_not_build_up_within_a_stroke() {
        let mut bm = page(100, 100, WHITE);
        // Out and back over the same line, plus a crossing.
        let s = stroke(
            Tool::Highlighter,
            Color::HIGHLIGHT,
            10.0,
            &[
                (10.0, 50.0),
                (90.0, 50.0),
                (50.0, 50.0),
                (50.0, 90.0),
                (50.0, 10.0),
            ],
        );
        draw_stroke(
            &mut bm,
            flip(),
            &s,
            Filter::Normal,
            &PressureCurve::default(),
        );
        let once = px(&bm, 20, 50);
        let crossed = px(&bm, 50, 50);
        assert_eq!(once, crossed);
        assert_ne!(once, WHITE);
        // A second stroke over it darkens it again, like a second pass.
        draw_stroke(
            &mut bm,
            flip(),
            &s,
            Filter::Normal,
            &PressureCurve::default(),
        );
        assert!(px(&bm, 20, 50)[2] < once[2]);
    }

    #[test]
    fn full_strength_highlighter_takes_the_swatch_colour_on_white_and_shows_on_dark() {
        let yellow = Color::rgb(0xff, 0xe9, 0x4f);
        let mut s = stroke(
            Tool::Highlighter,
            yellow,
            10.0,
            &[(10.0, 50.0), (90.0, 50.0)],
        );
        s.nib = scorecraft_doc::Nib::Chisel;
        let mut bm = page(100, 100, WHITE);
        draw_stroke(
            &mut bm,
            flip(),
            &s,
            Filter::Normal,
            &PressureCurve::default(),
        );
        assert_eq!(px(&bm, 50, 50), [0xff, 0xe9, 0x4f, 255]);
        let mut dark = page(100, 100, [0x1c, 0x1c, 0x1c, 255]);
        draw_stroke(
            &mut dark,
            flip(),
            &s,
            Filter::Dark,
            &PressureCurve::default(),
        );
        let p = px(&dark, 50, 50);
        assert!(p[0] > 100 && p[0] < 0xff, "laid over at 40%: {p:?}");
    }

    #[test]
    fn ink_6_highlighter_keeps_notation_black() {
        let mut bm = page(100, 100, [0, 0, 0, 255]);
        let s = stroke(
            Tool::Highlighter,
            Color::HIGHLIGHT,
            10.0,
            &[(10.0, 50.0), (90.0, 50.0)],
        );
        draw_stroke(
            &mut bm,
            flip(),
            &s,
            Filter::Normal,
            &PressureCurve::default(),
        );
        assert_eq!(px(&bm, 50, 50), [0, 0, 0, 255]);
    }

    #[test]
    fn ink_7_whiteout_covers_then_pen_shows_on_top() {
        let mut bm = page(100, 100, [0, 0, 0, 255]);
        let wo = stroke(
            Tool::Whiteout,
            Color::WHITE,
            20.0,
            &[(10.0, 50.0), (90.0, 50.0)],
        );
        let pen = stroke(Tool::Pen, Color::BLUE, 2.0, &[(50.0, 45.0), (50.0, 55.0)]);
        let (wo, pen) = (AnnotationKind::Stroke(wo), AnnotationKind::Stroke(pen));
        let out = compose(
            &bm.clone(),
            flip(),
            &[&wo, &pen],
            Filter::Normal,
            &PressureCurve::default(),
        )
        .unwrap();
        assert_eq!(px(&out, 30, 50), WHITE);
        assert_eq!(px(&out, 50, 50), [30, 80, 220, 255]);
        bm = out;
        assert_eq!(
            px(&bm, 30, 10),
            [0, 0, 0, 255],
            "outside the whiteout stays black"
        );
    }

    fn note(text: &str, background: Option<Color>) -> TextNote {
        TextNote {
            rect: Rect::new(10.0, 60.0, 95.0, 90.0),
            text: text.into(),
            style: scorecraft_doc::TextStyle {
                size: 12.0,
                color: Color::BLUE,
                background,
                ..Default::default()
            },
        }
    }

    #[test]
    fn text_notes_draw_their_letters_in_their_box() {
        let mut bm = page(100, 100, WHITE);
        draw_text(&mut bm, flip(), &note("Sample Text", None), Filter::Normal);
        let inked: Vec<(u32, u32)> = (0..100)
            .flat_map(|y| (0..100).map(move |x| (x, y)))
            .filter(|&(x, y)| px(&bm, x, y) != WHITE)
            .collect();
        assert!(inked.len() > 30, "letters drawn: {}", inked.len());
        // Inside the box: x 10..95, y (flipped) 10..40.
        assert!(
            inked
                .iter()
                .all(|&(x, y)| (9..=96).contains(&x) && (9..=41).contains(&y))
        );
        assert!(
            inked
                .iter()
                .any(|&(x, y)| px(&bm, x, y) == [30, 80, 220, 255])
        );
    }

    #[test]
    fn text_background_and_wrapping() {
        let mut bm = page(100, 100, [0, 0, 0, 255]);
        draw_text(
            &mut bm,
            flip(),
            &note("", Some(Color::WHITE)),
            Filter::Normal,
        );
        assert_eq!(px(&bm, 50, 25), WHITE);
        // Two lines: a narrow box wraps "Sample Text" onto a second line, 18 pt lower.
        let mut bm = page(100, 100, WHITE);
        let mut n = note("Sample Text", None);
        n.rect = Rect::new(10.0, 40.0, 55.0, 90.0);
        draw_text(&mut bm, flip(), &n, Filter::Normal);
        let rows: Vec<u32> = (0..100)
            .filter(|&y| (0..100).any(|x| px(&bm, x, y) != WHITE))
            .collect();
        assert!(
            rows.iter().any(|&y| y < 22) && rows.iter().any(|&y| y > 30),
            "{rows:?}"
        );
    }

    #[test]
    fn dynamics_and_hairpins_draw() {
        let mut bm = page(100, 100, WHITE);
        let mf = Stamp {
            glyph: 0,
            text: "mf".into(),
            size: 18.0,
            rect: Rect::new(20.0, 40.0, 50.0, 60.0),
            color: Color::BLACK,
        };
        draw_stamp(&mut bm, flip(), &mf, Filter::Normal);
        let dark = |bm: &Bitmap, x0: u32, x1: u32, y0: u32, y1: u32| {
            (y0..y1)
                .flat_map(|y| (x0..x1).map(move |x| (x, y)))
                .filter(|&(x, y)| px(bm, x, y) != WHITE)
                .count()
        };
        assert!(dark(&bm, 18, 52, 38, 62) > 20, "letters inside the box");
        assert_eq!(dark(&bm, 60, 100, 0, 100), 0);
        let h = Hairpin {
            from: Point::new(10.0, 20.0),
            to: Point::new(90.0, 20.0),
            crescendo: true,
            opening: 10.0,
            width: 1.5,
            color: Color::BLACK,
        };
        let [[o1, t1], [o2, t2]] = hairpin_lines(&h).unwrap();
        assert_eq!(
            (t1, t2),
            (h.from, h.from),
            "a crescendo points at its start"
        );
        assert!((o1.distance(o2) - 10.0).abs() < 1e-9);
        let mut bm = page(100, 100, WHITE);
        draw_hairpin(
            &mut bm,
            flip(),
            &h,
            Filter::Normal,
            &PressureCurve::default(),
        );
        // Open end: two lines 10 pt apart at x = 88, y = 15 and 25 (flipped: 85 and 75).
        assert_ne!(px(&bm, 88, 75), WHITE);
        assert_ne!(px(&bm, 88, 85), WHITE);
        assert_eq!(px(&bm, 88, 80), WHITE, "open between them");
        let dim = Hairpin {
            crescendo: false,
            ..h
        };
        assert_eq!(hairpin_lines(&dim).unwrap()[0][1], h.to);
        assert!(hairpin_lines(&Hairpin { to: h.from, ..h }).is_none());
    }

    #[test]
    fn ink_14_dark_filter_lightens_dark_ink() {
        assert_eq!(
            ink_color(Tool::Pen, Color::BLACK, Filter::Dark),
            Color::WHITE
        );
        let amber = Color::rgb(0xf2, 0xa9, 0x00);
        assert_eq!(
            ink_color(Tool::Pen, amber, Filter::Dark),
            amber,
            "light colours keep theirs"
        );
        let red = ink_color(Tool::Pen, Color::RED, Filter::Dark);
        assert!(
            red.r > 200 && red.g < 60 && red.b < 60,
            "red stays red: {red:?}"
        );
        let navy = ink_color(Tool::Pen, Color::rgb(0, 0, 128), Filter::Dark);
        assert!(navy.b > 200 && navy.r > 100, "{navy:?}");
        assert_eq!(
            ink_color(Tool::Pen, Color::BLACK, Filter::Sepia),
            Color::BLACK
        );
        let paper = Filter::Dark.colors().unwrap().paper;
        assert_eq!(
            ink_color(Tool::Whiteout, Color::WHITE, Filter::Dark),
            Color::rgb(paper[0], paper[1], paper[2])
        );
    }

    #[test]
    fn ink_11_hidden_layers_are_not_drawn() {
        use scorecraft_doc::{AnnotationId, Hlc, LayerId};
        let (l1, l2) = (LayerId::from(uuid(1)), LayerId::from(uuid(2)));
        let layers = vec![
            Layer {
                id: l1,
                name: "a".into(),
                visible: true,
                locked: false,
            },
            Layer {
                id: l2,
                name: "b".into(),
                visible: false,
                locked: false,
            },
        ];
        let ann = |id: u128, layer| Annotation {
            id: AnnotationId::from(uuid(id)),
            layer,
            stamp: Hlc::default(),
            kind: AnnotationKind::Stroke(stroke(Tool::Pen, Color::BLACK, 1.0, &[(0.0, 0.0)])),
        };
        let anns = vec![ann(10, l2), ann(11, l1), ann(12, LayerId::from(uuid(9)))];
        assert_eq!(visible_marks(&layers, &anns).len(), 1);
    }

    fn uuid(n: u128) -> uuid::Uuid {
        uuid::Uuid::from_u128(n)
    }

    #[test]
    fn page_mapping_follows_rotation() {
        let g = PageGeometry {
            width: 200.0,
            height: 100.0,
            rotation: 90,
            label: "1".into(),
            crop: [0.0, 0.0, 100.0, 200.0],
        };
        let bm = page(400, 200, WHITE);
        let m = page_to_bitmap(&g, &bm).unwrap();
        // The page's top-left corner (0, 200) lands at the bitmap's top right after a quarter turn.
        let p = m.apply(Point::new(0.0, 200.0));
        assert!((p.x - 400.0).abs() < 1e-9 && p.y.abs() < 1e-9, "{p:?}");
    }

    #[test]
    fn strokes_off_the_page_and_hostile_transforms() {
        let mut bm = page(10, 10, WHITE);
        let far = stroke(Tool::Pen, Color::BLACK, 2.0, &[(1e5, 1e5), (2e5, 2e5)]);
        draw_stroke(
            &mut bm,
            flip(),
            &far,
            Filter::Normal,
            &PressureCurve::default(),
        );
        assert!(bm.words.iter().all(|w| w.to_ne_bytes() == WHITE));
        let s = AnnotationKind::Stroke(stroke(Tool::Pen, Color::BLACK, 2.0, &[(1.0, 1.0)]));
        assert!(
            compose(
                &bm,
                Affine::new([f64::NAN; 6]),
                &[&s],
                Filter::Normal,
                &PressureCurve::default()
            )
            .is_none()
        );
        assert!(compose(&bm, flip(), &[], Filter::Normal, &PressureCurve::default()).is_none());
        // A stroke straddling the left edge clips.
        let edge = stroke(Tool::Pen, Color::BLACK, 4.0, &[(-5.0, 95.0), (5.0, 95.0)]);
        draw_stroke(
            &mut bm,
            flip(),
            &edge,
            Filter::Normal,
            &PressureCurve::default(),
        );
        assert_eq!(px(&bm, 0, 5), [0, 0, 0, 255]);
    }
}
