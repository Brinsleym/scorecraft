# ScoreCraft brand

The owner's logo (2026-11-08): the [app icon](../app-icon/README.md) beside the name, with the
tagline **SHEET MUSIC READER** under it.

| File | Use |
|---|---|
| `scorecraft-lockup-dark.svg` | On dark backgrounds: white name, zinc-400 tagline. The README header in dark mode |
| `scorecraft-lockup-light.svg` | On light backgrounds: ink name, zinc-500 tagline. The README header in light mode |
| `palette.svg` | The four brand colours as swatches |
| `make_brand.py` | Writes these and the icon masters in `../app-icon/` |

The lockups have transparent backgrounds and need no fonts: the text is outlined.

## Palette

| Name | Hex | Used for |
|---|---|---|
| ScoreCraft orange | `#c45821` | the icon tile; the accent in every theme (`theme::BRAND`) |
| Ink | `#18181B` | text on light; behind the pages in the dark theme (`theme::BRAND_INK`); the web build's background and theme colour |
| Surface | `#27272A` | raised dark surfaces: the dark reader's toolbars (`theme::BRAND_SURFACE`) |
| Paper | `#FFFFFF` | the note; text on the accent and on dark |

Two greys support them in the lockups: zinc-400 `#A1A1AA` and zinc-500 `#71717A` for the tagline.

## Type

- Name: **Space Grotesk Bold** (Florian Karsten, SIL OFL 1.1, `fonts/SpaceGrotesk-OFL.txt`).
- Tagline: **JetBrains Mono Regular**, tracked +0.2 em (JetBrains, SIL OFL 1.1,
  `fonts/JetBrainsMono-OFL.txt`).

Only outlines of the glyphs are in the SVGs; the font files aren't in the repository. To
regenerate, fetch `SpaceGrotesk[wght].ttf` and `JetBrainsMono[wght].ttf` from
[google/fonts](https://github.com/google/fonts) (`ofl/spacegrotesk`, `ofl/jetbrainsmono`), then:

```sh
pip install fonttools uharfbuzz
python3 assets/brand/make_brand.py --space-grotesk SpaceGrotesk[wght].ttf \
    --jetbrains-mono JetBrainsMono[wght].ttf
assets/app-icon/render.sh
```
