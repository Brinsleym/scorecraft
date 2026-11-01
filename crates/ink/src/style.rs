//! Tool colours and widths (spec ANNO-3, ANNO-4, ANNO-5).

use scorecraft_doc::{Color, Tool};

/// The palette button's colours (spec ANNO-4), six columns by five rows in reading order:
/// black, white and greys; reds, oranges and yellows; greens, cyan, blue and indigo; purples and
/// browns; pastels. ScoreCraft's own values. Swatches can only be set to these.
pub const PALETTE: [Color; 30] = [
    Color::rgb(0x00, 0x00, 0x00),
    Color::rgb(0xff, 0xff, 0xff),
    Color::rgb(0xd4, 0xd4, 0xd4),
    Color::rgb(0xa6, 0xa6, 0xa6),
    Color::rgb(0x80, 0x80, 0x80),
    Color::rgb(0x55, 0x55, 0x55),
    Color::rgb(0xc2, 0x18, 0x5b),
    Color::rgb(0xe5, 0x39, 0x35),
    Color::rgb(0xff, 0x57, 0x22),
    Color::rgb(0xff, 0xa0, 0x00),
    Color::rgb(0xff, 0xc4, 0x00),
    Color::rgb(0xff, 0xeb, 0x3b),
    Color::rgb(0xa4, 0xd6, 0x2c),
    Color::rgb(0x2e, 0xd8, 0x2e),
    Color::rgb(0x13, 0x8d, 0x5a),
    Color::rgb(0x0a, 0xa2, 0xc9),
    Color::rgb(0x1f, 0x5b, 0xd6),
    Color::rgb(0x3f, 0x1f, 0xbf),
    Color::rgb(0x7b, 0x2f, 0xbf),
    Color::rgb(0x5c, 0x1a, 0x7a),
    Color::rgb(0xf7, 0xd6, 0xc1),
    Color::rgb(0xc4, 0x9a, 0x6c),
    Color::rgb(0x9a, 0x5a, 0x2e),
    Color::rgb(0x6d, 0x4c, 0x41),
    Color::rgb(0xff, 0x8a, 0xe6),
    Color::rgb(0xff, 0xc9, 0x8a),
    Color::rgb(0xff, 0xf5, 0x9a),
    Color::rgb(0x8e, 0xff, 0xa8),
    Color::rgb(0x8a, 0xd8, 0xff),
    Color::rgb(0xc2, 0xc2, 0xff),
];

/// The eight toolbar swatches a new install starts with, in toolbar order: black, red, blue,
/// green, amber, purple, brown, grey. Each is a [`PALETTE`] colour.
pub const SWATCHES: [Color; 8] = [
    PALETTE[0],
    PALETTE[7],
    PALETTE[16],
    PALETTE[14],
    PALETTE[9],
    PALETTE[18],
    PALETTE[23],
    PALETTE[4],
];

/// The palette's white: the pen in this colour paints whiteout (spec ANNO-5).
pub const WHITEOUT_SWATCH: Color = PALETTE[1];

/// The highlighter's own palette (spec ANNO-4, screenshot 28), six columns by five rows: yellows
/// and golds; peaches, oranges and corals; pinks, roses, magenta and orchid; yellow-greens,
/// greens, mint and turquoise; blues, lavenders and violet. ScoreCraft's own values. Drawn at
/// full strength and multiplied with the page.
pub const HIGHLIGHTER_PALETTE: [Color; 30] = [
    Color::rgb(0xff, 0xf6, 0xb0),
    Color::rgb(0xff, 0xf0, 0x7a),
    Color::rgb(0xff, 0xe9, 0x4f),
    Color::rgb(0xff, 0xe6, 0x00),
    Color::rgb(0xff, 0xd2, 0x3f),
    Color::rgb(0xff, 0xbf, 0x1f),
    Color::rgb(0xff, 0xd9, 0xb3),
    Color::rgb(0xff, 0xc2, 0x85),
    Color::rgb(0xff, 0xa9, 0x40),
    Color::rgb(0xff, 0x8c, 0x4a),
    Color::rgb(0xff, 0x9e, 0x85),
    Color::rgb(0xff, 0x7a, 0x70),
    Color::rgb(0xff, 0xd1, 0xdc),
    Color::rgb(0xff, 0x9e, 0xc4),
    Color::rgb(0xff, 0x80, 0xb0),
    Color::rgb(0xff, 0x4f, 0x8f),
    Color::rgb(0xff, 0x66, 0xe0),
    Color::rgb(0xe8, 0x5a, 0xd0),
    Color::rgb(0xd9, 0xf0, 0x7a),
    Color::rgb(0xcd, 0xf0, 0x3c),
    Color::rgb(0x9c, 0xf0, 0x7a),
    Color::rgb(0x4f, 0xd9, 0x6b),
    Color::rgb(0x7a, 0xf0, 0xd0),
    Color::rgb(0x4f, 0xe0, 0xc8),
    Color::rgb(0xb3, 0xe5, 0xff),
    Color::rgb(0x85, 0xcc, 0xff),
    Color::rgb(0x66, 0xb3, 0xff),
    Color::rgb(0xd9, 0xb8, 0xff),
    Color::rgb(0xbf, 0x8c, 0xf0),
    Color::rgb(0xa3, 0x66, 0xff),
];

/// The highlighter's eight swatches on a new install: yellow, pink, light green, light blue,
/// orange, lavender, coral, aqua. Each is a [`HIGHLIGHTER_PALETTE`] colour.
pub const HIGHLIGHTER_SWATCHES: [Color; 8] = [
    HIGHLIGHTER_PALETTE[2],
    HIGHLIGHTER_PALETTE[13],
    HIGHLIGHTER_PALETTE[20],
    HIGHLIGHTER_PALETTE[25],
    HIGHLIGHTER_PALETTE[8],
    HIGHLIGHTER_PALETTE[28],
    HIGHLIGHTER_PALETTE[11],
    HIGHLIGHTER_PALETTE[22],
];

/// An inclusive range of widths in points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WidthRange {
    pub min: f32,
    pub max: f32,
}

impl WidthRange {
    /// `w` clamped into the range; a non-finite width becomes the minimum.
    pub fn clamp(self, w: f32) -> f32 {
        if w.is_finite() {
            w.clamp(self.min, self.max)
        } else {
            self.min
        }
    }

    pub fn contains(self, w: f32) -> bool {
        w.is_finite() && (self.min..=self.max).contains(&w)
    }
}

/// Pen widths (spec ANNO-4).
pub const PEN_WIDTHS: WidthRange = WidthRange {
    min: 0.25,
    max: 12.0,
};
/// Highlighter widths.
pub const HIGHLIGHTER_WIDTHS: WidthRange = WidthRange {
    min: 4.0,
    max: 40.0,
};
/// Whiteout widths.
pub const WHITEOUT_WIDTHS: WidthRange = WidthRange {
    min: 2.0,
    max: 40.0,
};
/// Eraser diameters.
pub const ERASER_WIDTHS: WidthRange = WidthRange {
    min: 2.0,
    max: 60.0,
};

/// A drawing tool's colour and width.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ToolStyle {
    pub color: Color,
    pub width: f32,
}

impl ToolStyle {
    /// What a tool draws with before the player changes it.
    pub fn default_for(tool: Tool) -> ToolStyle {
        match tool {
            Tool::Pen => ToolStyle {
                color: Color::BLACK,
                width: 1.5,
            },
            Tool::Highlighter => ToolStyle {
                color: HIGHLIGHTER_SWATCHES[0],
                width: 12.0,
            },
            Tool::Whiteout => ToolStyle {
                color: Color::WHITE,
                width: 10.0,
            },
        }
    }

    /// The width range of a tool.
    pub fn widths(tool: Tool) -> WidthRange {
        match tool {
            Tool::Pen => PEN_WIDTHS,
            Tool::Highlighter => HIGHLIGHTER_WIDTHS,
            Tool::Whiteout => WHITEOUT_WIDTHS,
        }
    }

    /// The colour `tool` uses when the player picks `picked`: the highlighter and whiteout are at
    /// full strength (the highlighter multiplies, so it never hides notation; spec ANNO-3).
    pub fn color_for(tool: Tool, picked: Color) -> Color {
        match tool {
            Tool::Pen => picked,
            Tool::Highlighter | Tool::Whiteout => Color::rgb(picked.r, picked.g, picked.b),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_inside_their_ranges() {
        for t in [Tool::Pen, Tool::Highlighter, Tool::Whiteout] {
            let s = ToolStyle::default_for(t);
            assert!(ToolStyle::widths(t).contains(s.width), "{t:?}");
        }
        assert!(ERASER_WIDTHS.contains(8.0));
    }

    #[test]
    fn highlighter_is_at_full_strength() {
        let c = ToolStyle::color_for(Tool::Highlighter, Color::rgba(1, 2, 3, 40));
        assert_eq!(c, Color::rgb(1, 2, 3));
        assert!(SWATCHES.iter().all(|c| PALETTE.contains(c)));
        assert!(
            HIGHLIGHTER_SWATCHES
                .iter()
                .all(|c| HIGHLIGHTER_PALETTE.contains(c))
        );
        assert_eq!(ToolStyle::color_for(Tool::Pen, SWATCHES[1]), SWATCHES[1]);
    }

    #[test]
    fn clamp_handles_bad_widths() {
        assert_eq!(PEN_WIDTHS.clamp(f32::NAN), 0.25);
        assert_eq!(PEN_WIDTHS.clamp(100.0), 12.0);
        assert!(!PEN_WIDTHS.contains(f32::INFINITY));
    }

    #[test]
    fn distinct_opaque_colours() {
        for list in [
            &SWATCHES[..],
            &PALETTE[..],
            &HIGHLIGHTER_SWATCHES[..],
            &HIGHLIGHTER_PALETTE[..],
        ] {
            for (i, a) in list.iter().enumerate() {
                assert_eq!(a.a, 255);
                for b in &list[i + 1..] {
                    assert_ne!(a, b);
                }
            }
        }
    }
}
