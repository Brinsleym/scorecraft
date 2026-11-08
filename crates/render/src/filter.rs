//! Display filters (architecture §7). Dark and sepia recolour the rendered page by lightness:
//! black becomes the ink colour, white the paper colour, and the greys between them in step. Done
//! on the pixels rather than on the PDF's paints, so a scanned score (one picture per page)
//! changes colour like an engraved one (owner, 2026-11-08: they did nothing on scans).

use scorecraft_pdf::{Bitmap, PaperColors};

/// How a page is coloured on screen. Display only; the score's file never changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Filter {
    /// The file's own colours.
    #[default]
    Normal,
    /// Light ink on a dark page, for dim stages.
    Dark,
    /// Dark brown ink on warm paper.
    Sepia,
}

impl Filter {
    pub const ALL: [Filter; 3] = [Filter::Normal, Filter::Dark, Filter::Sepia];

    /// The ink and paper colours the backend draws with, `None` for the file's own.
    pub fn colors(self) -> Option<PaperColors> {
        match self {
            Filter::Normal => None,
            Filter::Dark => Some(PaperColors {
                ink: [0xe6, 0xe6, 0xe6],
                paper: [0x1c, 0x1c, 0x1c],
            }),
            Filter::Sepia => Some(PaperColors {
                ink: [0x3b, 0x2a, 0x1a],
                paper: [0xf4, 0xec, 0xd8],
            }),
        }
    }

    /// The stable id used by settings, commands and the control channel.
    pub fn id(self) -> &'static str {
        match self {
            Filter::Normal => "normal",
            Filter::Dark => "dark",
            Filter::Sepia => "sepia",
        }
    }

    /// The filter with this [`Filter::id`].
    pub fn from_id(id: &str) -> Option<Filter> {
        Filter::ALL.into_iter().find(|f| f.id() == id)
    }

    /// Recolours `bm`, a page rendered in the file's own colours: each pixel's lightness picks
    /// its colour between [`PaperColors::ink`] (black) and [`PaperColors::paper`] (white).
    /// Colour in the file becomes a shade of the two. Nothing to do for [`Filter::Normal`].
    pub fn recolour(self, bm: &mut Bitmap) {
        let Some(c) = self.colors() else {
            return;
        };
        let ramp = ramp(c);
        for w in &mut bm.words {
            let [r, g, b, a] = w.to_ne_bytes();
            if a == 0 {
                continue;
            }
            // Rec. 709 lightness of the premultiplied colour, in 0..=255·a.
            let l = (u32::from(r) * 54 + u32::from(g) * 183 + u32::from(b) * 19) >> 8;
            let [r, g, b] = if a == 255 {
                ramp.get(l as usize).copied().unwrap_or(c.paper)
            } else {
                // Undo the premultiplication for the lookup, then premultiply the result.
                let straight = (l * 255 / u32::from(a)).min(255) as usize;
                let [r, g, b] = ramp.get(straight).copied().unwrap_or(c.paper);
                let m = |v: u8| ((u16::from(v) * u16::from(a) + 127) / 255) as u8;
                [m(r), m(g), m(b)]
            };
            *w = u32::from_ne_bytes([r, g, b, a]);
        }
    }
}

/// The colour for each lightness, 0 (black: the ink) to 255 (white: the paper).
fn ramp(c: PaperColors) -> [[u8; 3]; 256] {
    let mut out = [[0u8; 3]; 256];
    for (l, slot) in out.iter_mut().enumerate() {
        let t = l as u32;
        for ((o, ink), paper) in slot.iter_mut().zip(c.ink).zip(c.paper) {
            let v = (u32::from(ink) * (255 - t) + u32::from(paper) * t + 127) / 255;
            *o = v.min(255) as u8;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{Bitmap, Filter};

    #[test]
    fn ids_round_trip() {
        for f in Filter::ALL {
            assert_eq!(Filter::from_id(f.id()), Some(f));
        }
        assert_eq!(Filter::from_id("neon"), None);
        assert_eq!(Filter::default().colors(), None);
    }

    fn bitmap(pixels: &[[u8; 4]]) -> Bitmap {
        Bitmap {
            width: pixels.len() as u32,
            height: 1,
            words: pixels.iter().map(|p| u32::from_ne_bytes(*p)).collect(),
        }
    }

    #[test]
    fn recolour_maps_black_to_ink_and_white_to_paper() {
        let mut bm = bitmap(&[
            [0, 0, 0, 255],
            [255, 255, 255, 255],
            [128, 128, 128, 255],
            [0, 0, 0, 0],
            // Half-covered white, premultiplied.
            [128, 128, 128, 128],
        ]);
        let normal = bm.clone();
        Filter::Normal.recolour(&mut bm);
        assert_eq!(bm, normal);
        Filter::Dark.recolour(&mut bm);
        let c = Filter::Dark.colors().unwrap();
        let px = |i: u32| bm.pixel(i, 0).unwrap();
        assert_eq!(px(0)[..3], c.ink);
        assert_eq!(px(1)[..3], c.paper);
        // Mid grey lands between the two.
        assert!(px(2)[0] > c.paper[0] && px(2)[0] < c.ink[0]);
        assert_eq!(px(3), [0, 0, 0, 0]);
        // Premultiplied paper at half cover.
        assert_eq!(px(4), [14, 14, 14, 128]);
    }
}
