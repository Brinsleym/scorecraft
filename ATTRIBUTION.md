# Attribution

Every non-code asset in this repository, with its author, source and licence. ScoreCraft's own code
and original assets are MIT OR Apache-2.0 (see [`LICENSE-MIT`](LICENSE-MIT),
[`LICENSE-APACHE`](LICENSE-APACHE) and [`NOTICE`](NOTICE)). When you add an asset, add a row here in
the same change; third-party assets must be permissively licensed and keep their licence file next
to them ([`docs/contributing.md`](docs/contributing.md#adding-an-asset)).

## Bundled in the app

| Path | Title | Author | Source | License |
|---|---|---|---|---|
| _none yet_ | | | | |

## Used by tests and docs only

| Path | Title | Author | Source | License |
|---|---|---|---|---|
| _none yet_ | | | | |

## Reused code (dependencies, not copied files)

| Component | Source | License | Use |
|---|---|---|---|
| PdfCraft crates (`pdfcraft-cos`, `-render`, `-annot`, `-create`, `-organize`, `-print`, `-ocr` and their lower layers) | <https://github.com/storytold/pdfcraft>, pinned at `65ff3a00` (0.15.0); builds include `-render`, `-cos`, `-organize`, `-content`, `-crypt`, `-filters`, `-fonts` so far | MIT OR Apache-2.0 | The PDF backend ([architecture §15](docs/architecture.md#15-reusing-pdfcraft)) |
| hayro family as patched in PdfCraft's `vendor/` | same | Apache-2.0 OR MIT; bundled Foxit fonts and Adobe CMaps BSD-3-Clause | Page rasterisation, through `pdfcraft-render` |
| lopdf, `vello_common`, `vello_cpu` as patched in PdfCraft's `vendor/` | same | MIT; Apache-2.0 OR MIT | PDF object model and CPU rasteriser, through `pdfcraft-render` |
| Inter and Dancing Script fonts embedded by `pdfcraft-fonts` | same | SIL OFL 1.1 | Fallback fonts for PDFs that don't embed theirs; Inter Regular also sets ScoreCraft's text notes and stands in for Edwin (synthetic bold italic) |

Notices for these are in [`NOTICE`](NOTICE).

## Process credits

The development process, document structure and many rules in this repository are adapted from
the storytold Crafting Apps, in particular [storytold/photocraft](https://github.com/storytold/photocraft)
(MIT OR Apache-2.0) and [storytold/craft-contrib](https://github.com/storytold/craft-contrib). The
text here is rewritten for ScoreCraft; no code was copied.
