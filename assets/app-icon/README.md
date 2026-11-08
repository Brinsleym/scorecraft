# ScoreCraft app icon

A **white eighth note on a ScoreCraft-orange rounded square**: the owner's design (2026-11-08),
redrawn as plain geometry.

## Palette

| Colour | Hex | Used for |
|---|---|---|
| ScoreCraft orange (app colour) | `#c45821` | the full-bleed tile |
| Paper | `#FFFFFF` | the note |

The full brand palette and the lockups are in [`../brand/`](../brand/README.md).

## Geometry

A 512-unit tile (`viewBox="0 0 512 512"`), rounded square with `rx=112` and no border, the Crafting
Apps' tile. The note is one path: a round head (r 52) at (228, 312), a 26-unit stem from its right
edge up to y 148, and a square flag 82 × 40 with 8-unit corners. It sits optically centred and
inside both the web's maskable safe circle and Android's 66 dp adaptive safe zone. The macOS render
pads the tile onto Apple's 824/1024 icon grid; the Windows, Linux, web and Android renders use the
whole tile.

## Files

- `scorecraft.svg`: canonical master. Also the hicolor scalable icon and the web favicon.
- `scorecraft-glyph.svg`: the note alone, black on transparent, for monochrome and tray uses.
- `scorecraft-1024.png`: 1024 px render on the macOS grid; the window icon on macOS.
- `scorecraft.ico`: Windows icon, 16–256 px, embedded in `scorecraft.exe` by
  `apps/scorecraft/build.rs`.
- `hicolor/<size>/apps/app.scorecraft.png` (16–512) and `hicolor/scalable/apps/app.scorecraft.svg`:
  Linux icon theme; the 256 px render is the window icon elsewhere.

Platform copies rendered from the same master:

- `apps/scorecraft-web/favicon.svg`, `icons/` and `manifest.webmanifest`: favicon, PNG fallback,
  install icons (192, 512, maskable 512) and the iOS home-screen icon.
- `apps/scorecraft-android/res/`: legacy `mipmap-*/ic_launcher.png` and the adaptive icon
  (`mipmap-anydpi-v26`, orange background, the note as a vector foreground and themed monochrome).

The app sets the window icon and app id (`app.scorecraft`) at runtime
(`apps/scorecraft/src/app_icon.rs`, `main.rs`), whose tests check the `.ico` sizes and the render.

## Regenerate

Edit the geometry in `assets/brand/make_brand.py` and run it (it rewrites the SVG masters and the
lockups), then run `assets/app-icon/render.sh`. That needs `resvg` (`cargo install resvg`); it ends
with `cargo xtask ico`, which packs the `.ico`. If you change the note, update the path in
`apps/scorecraft-android/res/drawable/ic_launcher_foreground.xml` too.

## Provenance

The owner's original mockup (2026-11-08), redrawn as vectors. Not derived from enScore's or any
other app's icon ([clean-room §3](../../docs/clean-room.md)). License: MIT OR Apache-2.0, like the
rest of ScoreCraft.
