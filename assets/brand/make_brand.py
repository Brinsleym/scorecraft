#!/usr/bin/env python3
"""Writes ScoreCraft's vector brand files: the app icon, the lockups and the palette card.

The icon is plain geometry. The lockups' text is outlined from two SIL OFL fonts (Space Grotesk
Bold for the name, JetBrains Mono Regular for the tagline), so the SVGs need no font installed.
Run from anywhere:

    pip install fonttools uharfbuzz
    python3 assets/brand/make_brand.py --space-grotesk SpaceGrotesk[wght].ttf \
        --jetbrains-mono JetBrainsMono[wght].ttf

Then `assets/app-icon/render.sh` renders the PNGs, the .ico and the platform icons.
"""

import argparse
import io
from pathlib import Path

import uharfbuzz as hb
from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.pens.transformPen import TransformPen
from fontTools.ttLib import TTFont
from fontTools.varLib.instancer import instantiateVariableFont

ROOT = Path(__file__).resolve().parents[2]

BRAND = "#c45821"  # ScoreCraft orange: the tile, the accent
INK = "#18181B"  # zinc-900: dark surfaces, text on light
SURFACE = "#27272A"  # zinc-800: raised dark surfaces
PAPER = "#FFFFFF"  # the note, text on dark
MUTED_DARK = "#A1A1AA"  # zinc-400: the tagline on dark
MUTED_LIGHT = "#71717A"  # zinc-500: the tagline on light

# The eighth note on the 512-unit tile: a round head, a stem rising from its right edge and a
# square flag. Kept as one path (arcs, no <circle>) so Android's vector drawable can reuse it.
NOTE = (
    "M176 312a52 52 0 1 1 104 0a52 52 0 1 1-104 0Z"
    "M254 312V156q0-8 8-8h66q8 0 8 8v24q0 8-8 8h-48v124Z"
)
TILE_RX = 112  # the Crafting Apps' rounded square (photocraft assets/app-icon/README.md)


def icon_svg(tile=True):
    bg = f'<rect width="512" height="512" rx="{TILE_RX}" fill="{BRAND}"/>' if tile else ""
    return (
        '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512" width="512" height="512">'
        f"<title>ScoreCraft</title>{bg}"
        f'<path fill="{PAPER if tile else "#000"}" d="{NOTE}"/></svg>\n'
    )


def load(path, wght):
    font = TTFont(path)
    if "fvar" in font:
        font = instantiateVariableFont(font, {"wght": wght})
    buf = io.BytesIO()
    font.save(buf)
    data = buf.getvalue()
    return TTFont(io.BytesIO(data)), data


def outline(font, data, text, size, x, y, tracking=0.0):
    """SVG path data for `text` set at `size` px with its baseline at (x, y); returns (d, width)."""
    face = hb.Face(data)
    hbfont = hb.Font(face)
    buf = hb.Buffer()
    buf.add_str(text)
    buf.guess_segment_properties()
    hb.shape(hbfont, buf, {"kern": True, "liga": True})
    upem = font["head"].unitsPerEm
    scale = size / upem
    glyphs = font.getGlyphSet()
    names = font.getGlyphOrder()
    pen = SVGPathPen(glyphs, ntos=lambda v: f"{v:.2f}".rstrip("0").rstrip("."))
    cursor = 0.0
    n = len(buf.glyph_infos)
    for i, (info, pos) in enumerate(zip(buf.glyph_infos, buf.glyph_positions)):
        name = names[info.codepoint]
        gx = x + (cursor + pos.x_offset) * scale
        gy = y - pos.y_offset * scale
        glyphs[name].draw(TransformPen(pen, (scale, 0, 0, -scale, gx, gy)))
        cursor += pos.x_advance
        if i < n - 1:
            cursor += tracking * upem
    return pen.getCommands(), cursor * scale


def lockup(name_font, mono_font, dark):
    # A 128-unit tile, the name beside it, the tagline under the name (the owner's mockup).
    name_fill = PAPER if dark else INK
    tag_fill = MUTED_DARK if dark else MUTED_LIGHT
    name_d, name_w = outline(*name_font, "ScoreCraft", 64, 164, 66)
    tag_d, tag_w = outline(*mono_font, "SHEET MUSIC READER", 24, 166, 114, tracking=0.2)
    width = round(164 + max(name_w, tag_w) + 4)
    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width} 128" width="{width * 2}" '
        f'height="256"><title>ScoreCraft: sheet music reader</title>'
        f'<rect width="128" height="128" rx="28" fill="{BRAND}"/>'
        f'<path fill="{PAPER}" transform="scale(0.25)" d="{NOTE}"/>'
        f'<path fill="{name_fill}" d="{name_d}"/>'
        f'<path fill="{tag_fill}" d="{tag_d}"/></svg>\n'
    )


def palette():
    swatches = [(BRAND, "Brand"), (INK, "Ink"), (SURFACE, "Surface"), (PAPER, "Paper")]
    parts = []
    for i, (hex_, _) in enumerate(swatches):
        x = 24 + i * 120
        edge = ' stroke="#3F3F46" stroke-width="2"' if hex_ in (INK, SURFACE) else ""
        parts.append(f'<rect x="{x}" y="24" width="96" height="96" rx="22" fill="{hex_}"{edge}/>')
    return (
        '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 504 144" width="504" height="144">'
        f'<title>ScoreCraft palette: {", ".join(h for h, _ in swatches)}</title>'
        f'<rect width="504" height="144" rx="16" fill="#111113"/>{"".join(parts)}</svg>\n'
    )


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--space-grotesk", required=True)
    ap.add_argument("--jetbrains-mono", required=True)
    a = ap.parse_args()
    name_font = load(a.space_grotesk, 700)
    mono_font = load(a.jetbrains_mono, 400)
    out = {
        ROOT / "assets/app-icon/scorecraft.svg": icon_svg(),
        ROOT / "assets/app-icon/scorecraft-glyph.svg": icon_svg(tile=False),
        ROOT / "assets/brand/scorecraft-lockup-dark.svg": lockup(name_font, mono_font, True),
        ROOT / "assets/brand/scorecraft-lockup-light.svg": lockup(name_font, mono_font, False),
        ROOT / "assets/brand/palette.svg": palette(),
    }
    for path, text in out.items():
        path.write_text(text, encoding="utf-8")
        print(path.relative_to(ROOT))


if __name__ == "__main__":
    main()
