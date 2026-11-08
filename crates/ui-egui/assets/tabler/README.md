# Tabler Icons subset

`tabler-icons.ttf` is `dist/fonts/tabler-icons.ttf` from `@tabler/icons-webfont@3.31.0` (MIT,
[`LICENSE`](LICENSE)), cut to the glyphs `src/icons.rs` uses so the app carries about 12 KB instead
of 2.3 MB. Outline icons at Tabler's default stroke of 2.

To add an icon: find its codepoint in that package's `dist/tabler-icons.min.css`
(`.ti-<name>:before{content:"\<hex>"}`), add a variant and its `glyph()` arm in `src/icons.rs`,
then re-cut the subset with every codepoint in `glyph()` (fontTools' `pyftsubset`, a dev tool,
not a build step):

```sh
pyftsubset tabler-icons.ttf --unicodes=U+ea19,U+ea5c,... \
  --output-file=crates/ui-egui/assets/tabler/tabler-icons.ttf --no-hinting --layout-features=''
```

Keep the version in step with `ATTRIBUTION.md`.
