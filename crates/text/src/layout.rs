//! Text-note layout (spec ANNO-8): wrapping to a maximum width, the box a note takes, and
//! keeping that box inside the page.

use scorecraft_geom::Rect;

use crate::Face;

/// A note wraps at this share of the page's width (measured from enScore, screenshot 29).
pub const NOTE_MAX_WIDTH: f64 = 0.86;
/// Lines are this many times the text size apart.
pub const LINE_SPACING: f64 = 1.5;
/// The longest note, in characters.
pub const MAX_TEXT_CHARS: usize = 5000;
/// The most lines laid out.
const MAX_LINES: usize = 1000;

/// A laid-out note: its lines and the box they need, in points.
#[derive(Debug, Clone, PartialEq)]
pub struct Laid {
    pub lines: Vec<String>,
    /// The longest line's width.
    pub width: f64,
    /// Lines × [`LINE_SPACING`] × size.
    pub height: f64,
    pub size: f64,
    /// From the top of each line's box down to its baseline.
    pub baseline: f64,
}

/// `text` broken into lines no wider than `max_width` at `size`: at line breaks, then at spaces;
/// a word wider than the line breaks between characters. Empty paragraphs stay as empty lines.
pub fn wrap(face: &Face, text: &str, size: f64, max_width: f64) -> Vec<String> {
    let max_width = if max_width.is_finite() {
        max_width.max(0.0)
    } else {
        0.0
    };
    let text: String = text.chars().take(MAX_TEXT_CHARS).collect();
    let mut lines: Vec<String> = Vec::new();
    for para in text.split('\n') {
        let mut line = String::new();
        for word in para.split(' ') {
            let candidate = if line.is_empty() {
                word.to_owned()
            } else {
                format!("{line} {word}")
            };
            if face.advance(&candidate, size) <= max_width || candidate.is_empty() {
                line = candidate;
                continue;
            }
            if !line.is_empty() {
                lines.push(std::mem::take(&mut line));
            }
            // The word alone: break it between characters if it is still too wide.
            for c in word.chars() {
                let mut next = line.clone();
                next.push(c);
                if !line.is_empty() && face.advance(&next, size) > max_width {
                    lines.push(std::mem::take(&mut line));
                    line.push(c);
                } else {
                    line = next;
                }
            }
            if lines.len() >= MAX_LINES {
                break;
            }
        }
        lines.push(line);
        if lines.len() >= MAX_LINES {
            lines.truncate(MAX_LINES);
            break;
        }
    }
    lines
}

/// `text` laid out at `size` within `max_width`.
pub fn layout(face: &Face, text: &str, size: f64, max_width: f64) -> Laid {
    let lines = wrap(face, text, size, max_width);
    let width = lines
        .iter()
        .map(|l| face.advance(l, size))
        .fold(0.0, f64::max);
    let line = size * LINE_SPACING;
    let (asc, desc) = face.ascent_descent(size);
    Laid {
        height: line * lines.len().max(1) as f64,
        // The text centred in each line's box.
        baseline: (line + asc - desc) / 2.0,
        lines,
        width,
        size,
    }
}

/// `rect` moved by the least distance that puts it inside `page`. A box larger than the page
/// keeps its top-left corner at the page's top-left (page space, y up: top is `y1`).
pub fn fit_inside(rect: Rect, page: Rect) -> Rect {
    let (w, h) = (rect.width(), rect.height());
    let x0 = if w >= page.width() {
        page.x0
    } else {
        rect.x0.clamp(page.x0, page.x1 - w)
    };
    let y1 = if h >= page.height() {
        page.y1
    } else {
        rect.y1.clamp(page.y0 + h, page.y1)
    };
    Rect::new(x0, y1 - h, x0 + w, y1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FaceKind;

    fn face() -> Face {
        Face::get(FaceKind::Note, false, false).unwrap()
    }

    #[test]
    fn ink_33_wraps_at_the_maximum_width() {
        let f = face();
        let text = "Sample Text ".repeat(40);
        let max = 612.0 * NOTE_MAX_WIDTH;
        let laid = layout(&f, text.trim(), 12.0, max);
        assert!(laid.lines.len() > 3);
        assert!(
            laid.width <= max + 1e-9 && laid.width > max - 70.0,
            "{}",
            laid.width
        );
        for l in &laid.lines {
            assert!(
                !l.starts_with(' ') && f.advance(l, 12.0) <= max + 1e-9,
                "{l:?}"
            );
        }
        assert!((laid.height - laid.lines.len() as f64 * 18.0).abs() < 1e-9);
        assert!(laid.baseline > 9.0 && laid.baseline < 18.0);
    }

    #[test]
    fn short_notes_are_as_wide_as_their_text_and_long_words_break() {
        let f = face();
        let a = layout(&f, "a", 12.0, 500.0);
        assert_eq!(a.lines, vec!["a".to_owned()]);
        assert!(a.width < 10.0);
        let long = wrap(&f, &"m".repeat(200), 12.0, 100.0);
        assert!(long.len() > 10);
        assert!(long.iter().all(|l| f.advance(l, 12.0) <= 100.0 + 1e-9));
        assert_eq!(wrap(&f, "one\n\ntwo", 12.0, 500.0), vec!["one", "", "two"]);
    }

    #[test]
    fn notes_stay_inside_the_page() {
        let page = Rect::new(0.0, 0.0, 612.0, 792.0);
        let near_right = fit_inside(Rect::new(600.0, 100.0, 700.0, 118.0), page);
        assert_eq!((near_right.x1, near_right.x0), (612.0, 512.0));
        let above = fit_inside(Rect::new(10.0, 790.0, 50.0, 820.0), page);
        assert_eq!(above.y1, 792.0);
        let low = fit_inside(Rect::new(10.0, -20.0, 50.0, 10.0), page);
        assert_eq!(low.y0, 0.0);
        let huge = fit_inside(Rect::new(300.0, 300.0, 1300.0, 1300.0), page);
        assert_eq!((huge.x0, huge.y1), (0.0, 792.0));
        let corner = fit_inside(Rect::new(0.0, 774.0, 80.0, 792.0), page);
        assert_eq!(corner, Rect::new(0.0, 774.0, 80.0, 792.0));
    }

    #[test]
    fn hostile_sizes_and_lengths_are_bounded() {
        let f = face();
        let big = "x ".repeat(100_000);
        assert!(wrap(&f, &big, 12.0, 1.0).len() <= MAX_LINES);
        let _ = layout(&f, "x", f64::NAN, f64::INFINITY);
        let _ = wrap(&f, "x", 12.0, f64::NAN);
    }
}
