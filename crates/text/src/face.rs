//! Font faces: advances, vertical metrics and glyph outlines.

use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::{FontRef, GlyphId, MetadataProvider};

use scorecraft_geom::Point;

include!(concat!(env!("OUT_DIR"), "/fonts.rs"));

/// Which face to draw with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaceKind {
    /// Text notes: Inter Regular (spec ANNO-8).
    Note,
    /// Dynamics: Edwin Bold Italic, or Inter slanted and emboldened without it (spec ANNO-9).
    Dynamic,
}

/// A loaded face with an optional synthetic slant and weight.
#[derive(Clone)]
pub struct Face {
    font: FontRef<'static>,
    /// Horizontal shear per unit of height (0.2 ≈ 11°), for italic without an italic font.
    slant: f64,
    /// Extra width per side as a fraction of the size, for bold without a bold font.
    embolden: f64,
    /// Whether this is the requested face (false: a fallback stands in).
    exact: bool,
}

impl std::fmt::Debug for Face {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Face")
            .field("slant", &self.slant)
            .field("embolden", &self.embolden)
            .field("exact", &self.exact)
            .finish_non_exhaustive()
    }
}

/// Synthetic italic shear.
const ITALIC_SLANT: f64 = 0.2;
/// Synthetic bold: each glyph is drawn again this far to the right (fraction of the size).
const BOLD_OFFSET: f64 = 0.035;
/// Longest outline segment when flattening curves, as a fraction of the size.
const FLATNESS: f64 = 0.02;
/// The most points one glyph's outline may have after flattening.
const MAX_GLYPH_POINTS: usize = 20_000;

impl Face {
    /// The face for `kind`, with synthetic `bold` and `italic` on top. `None` only if the
    /// built-in font fails to parse, which a test rules out.
    pub fn get(kind: FaceKind, bold: bool, italic: bool) -> Option<Face> {
        let inter = FontRef::new(scorecraft_pdf::interface_font()).ok()?;
        let (font, mut slant, mut embolden, exact) = match kind {
            FaceKind::Note => (inter, 0.0, 0.0, true),
            FaceKind::Dynamic => match EDWIN_BOLD_ITALIC.and_then(|b| FontRef::new(b).ok()) {
                Some(edwin) => (edwin, 0.0, 0.0, true),
                None => (inter, ITALIC_SLANT, BOLD_OFFSET, false),
            },
        };
        if italic {
            slant = ITALIC_SLANT;
        }
        if bold {
            embolden = BOLD_OFFSET;
        }
        Some(Face {
            font,
            slant,
            embolden,
            exact,
        })
    }

    /// Whether the requested font is in this build (false: the fallback stands in).
    pub fn is_exact(&self) -> bool {
        self.exact
    }

    fn glyph(&self, c: char) -> GlyphId {
        self.font.charmap().map(c).unwrap_or(GlyphId::NOTDEF)
    }

    fn advance_of(&self, g: GlyphId, size: f64) -> f64 {
        let m = self
            .font
            .glyph_metrics(Size::new(size as f32), LocationRef::default());
        f64::from(m.advance_width(g).unwrap_or(0.0)) + self.embolden * size
    }

    /// The width of `text` set at `size` points. Control characters take no space.
    pub fn advance(&self, text: &str, size: f64) -> f64 {
        if !(size.is_finite() && size > 0.0) {
            return 0.0;
        }
        text.chars()
            .filter(|c| !c.is_control())
            .map(|c| self.advance_of(self.glyph(c), size))
            .sum()
    }

    /// Distance from the baseline up to the top of tall letters, and down to the bottom of
    /// descenders, at `size` points (both positive).
    pub fn ascent_descent(&self, size: f64) -> (f64, f64) {
        let m = self
            .font
            .metrics(Size::new(size as f32), LocationRef::default());
        let a = f64::from(m.ascent);
        let d = f64::from(m.descent).abs();
        if a.is_finite() && d.is_finite() && a > 0.0 {
            (a, d)
        } else {
            (size * 0.8, size * 0.2)
        }
    }

    /// The outlines of one line of `text` at `size` points with its baseline starting at
    /// `origin` (page space, y up), as closed polygons with the font's own winding, for a
    /// non-zero fill.
    pub fn outlines(&self, text: &str, size: f64, origin: Point) -> Vec<Vec<Point>> {
        let mut out = Vec::new();
        if !(size.is_finite() && size > 0.0 && origin.is_finite()) {
            return out;
        }
        let glyphs = self.font.outline_glyphs();
        let settings = || DrawSettings::unhinted(Size::new(size as f32), LocationRef::default());
        let mut x = origin.x;
        for c in text.chars().filter(|c| !c.is_control()) {
            let g = self.glyph(c);
            if let Some(outline) = glyphs.get(g) {
                let passes: &[f64] = if self.embolden > 0.0 {
                    &[0.0, 1.0]
                } else {
                    &[0.0]
                };
                for pass in passes {
                    let mut pen = Flatten {
                        origin: Point::new(x + pass * self.embolden * size, origin.y),
                        slant: self.slant,
                        step: (size * FLATNESS).max(0.01),
                        current: Vec::new(),
                        contours: Vec::new(),
                        points: 0,
                    };
                    if outline.draw(settings(), &mut pen).is_ok() {
                        pen.flush();
                        out.extend(pen.contours);
                    }
                }
            }
            x += self.advance_of(g, size);
        }
        out
    }
}

/// An outline pen that flattens curves into polygons in page space.
struct Flatten {
    origin: Point,
    slant: f64,
    step: f64,
    current: Vec<Point>,
    contours: Vec<Vec<Point>>,
    points: usize,
}

impl Flatten {
    fn map(&self, x: f32, y: f32) -> Point {
        let (x, y) = (f64::from(x), f64::from(y));
        Point::new(self.origin.x + x + y * self.slant, self.origin.y + y)
    }

    fn push(&mut self, p: Point) {
        if self.points < MAX_GLYPH_POINTS && p.is_finite() {
            self.current.push(p);
            self.points += 1;
        }
    }

    fn last(&self) -> Point {
        self.current.last().copied().unwrap_or(self.origin)
    }

    fn pieces(&self, a: Point, b: Point) -> usize {
        ((a.distance(b) / self.step).ceil() as usize).clamp(1, 64)
    }

    fn flush(&mut self) {
        let c = std::mem::take(&mut self.current);
        if c.len() >= 3 {
            self.contours.push(c);
        }
    }
}

impl OutlinePen for Flatten {
    fn move_to(&mut self, x: f32, y: f32) {
        self.flush();
        let p = self.map(x, y);
        self.push(p);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let p = self.map(x, y);
        self.push(p);
    }

    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        let (p0, c, p1) = (self.last(), self.map(cx0, cy0), self.map(x, y));
        let n = self.pieces(p0, c) + self.pieces(c, p1);
        for k in 1..=n {
            let t = k as f64 / n as f64;
            let u = 1.0 - t;
            self.push(Point::new(
                u * u * p0.x + 2.0 * u * t * c.x + t * t * p1.x,
                u * u * p0.y + 2.0 * u * t * c.y + t * t * p1.y,
            ));
        }
    }

    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        let (p0, a, b, p1) = (
            self.last(),
            self.map(cx0, cy0),
            self.map(cx1, cy1),
            self.map(x, y),
        );
        let n = self.pieces(p0, a) + self.pieces(a, b) + self.pieces(b, p1);
        for k in 1..=n {
            let t = k as f64 / n as f64;
            let u = 1.0 - t;
            let (w0, w1, w2, w3) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
            self.push(Point::new(
                w0 * p0.x + w1 * a.x + w2 * b.x + w3 * p1.x,
                w0 * p0.y + w1 * a.y + w2 * b.y + w3 * p1.y,
            ));
        }
    }

    fn close(&mut self) {
        self.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note() -> Face {
        Face::get(FaceKind::Note, false, false).unwrap()
    }

    #[test]
    fn inter_measures_and_draws() {
        let f = note();
        let w = f.advance("Sample Text", 12.0);
        assert!(w > 50.0 && w < 90.0, "{w}");
        assert!((f.advance("Sample Text", 24.0) - 2.0 * w).abs() < 1e-6);
        let (a, d) = f.ascent_descent(12.0);
        assert!(a > 8.0 && a < 14.0 && d > 1.0 && d < 5.0, "{a} {d}");
        let o = f.outlines("Ag", 12.0, Point::new(100.0, 200.0));
        assert!(o.len() >= 3, "A has two contours, g at least one");
        for c in &o {
            for p in c {
                assert!(
                    p.x > 99.0 && p.x < 120.0 && p.y > 195.0 && p.y < 210.0,
                    "{p:?}"
                );
            }
        }
    }

    #[test]
    fn synthetic_styles_slant_and_widen() {
        let plain = note();
        let bold = Face::get(FaceKind::Note, true, false).unwrap();
        assert!(bold.advance("ff", 20.0) > plain.advance("ff", 20.0));
        let italic = Face::get(FaceKind::Note, false, true).unwrap();
        let top = |f: &Face| {
            f.outlines("l", 20.0, Point::new(0.0, 0.0))
                .iter()
                .flatten()
                .fold(f64::NEG_INFINITY, |m, p| m.max(p.x))
        };
        assert!(top(&italic) > top(&plain) + 2.0);
    }

    #[test]
    fn dynamic_face_falls_back_without_edwin() {
        let d = Face::get(FaceKind::Dynamic, false, false).unwrap();
        assert_eq!(d.is_exact(), EDWIN_BOLD_ITALIC.is_some());
        assert!(d.advance("mf", 18.0) > 0.0);
    }

    #[test]
    fn hostile_input_draws_nothing_or_boxes() {
        let f = note();
        assert!(f.outlines("x", f64::NAN, Point::new(0.0, 0.0)).is_empty());
        assert!(
            f.outlines("x", 12.0, Point::new(f64::INFINITY, 0.0))
                .is_empty()
        );
        assert_eq!(f.advance("x", -1.0), 0.0);
        assert_eq!(f.advance("\u{7}\n", 12.0), 0.0);
        // A character Inter lacks draws its missing-glyph box rather than nothing.
        assert!(f.advance("\u{10FFFD}", 12.0) > 0.0);
    }
}
