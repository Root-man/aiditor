# Design

## Context

GPUI 0.2.2 on Linux shapes with cosmic-text 0.14 and rasterizes with swash 0.2.10 (sources: COLR outline, colour bitmap, outline). Swash reads only COLR v0, so a COLRv1 glyph falls through to its base glyph's outline, which is empty: a scratch swash probe drew 0x0 masks for 🦀 😀 👍 🚀 ❤ ✅ ⚡ 🇺 from `Noto-COLRv1.ttf`, and real masks from `NotoEmoji-Regular.ttf` and `Symbola.ttf`. GPUI ignores `Font::fallbacks` on Linux, and cosmic-text's Unix fallback list names `Noto Color Emoji` before falling back to any font in the database. GPUI also can't name a font it fell back to (`get_font_for_id` knows only fonts resolved by name).

GPUI's Linux `load_family` removes from cosmic-text's database any face it loads by name that has no `m` glyph. Colour emoji fonts and Noto Emoji have none, so naming them in a run deletes them.

## Goals / Non-Goals

**Goals:** every emoji visible on Linux with a COLRv1-only colour font; colour kept wherever GPUI can draw it.

**Non-Goals:** drawing COLRv1 in colour (needs a COLRv1 rasterizer in GPUI); bundling a monochrome font when none is installed; choosing which installed font draws the emoji.

## Decisions

- **Check the file once at start.** Ask `fc-match` for `Noto Color Emoji`'s file and read its table directory: GPUI can't draw it when it has a COLR table with no v0 base glyphs and no CBDT or sbix. No `fc-match` (macOS), an unreadable file or a collection leaves everything as it is. Alternatives: always dropping it on Linux loses colour on distros with CBDT Noto Color Emoji; telling the builds apart by units per em is a fingerprint, not a property.
- **Drop the font from GPUI's database.** Resolving `Noto Color Emoji` by name triggers `load_family`'s removal, so cosmic-text's fallback skips it and finds the next font with the glyph. That covers every text path at once (window lines, terminal runs, mode and echo lines, element text) with no change to shaping. It relies on a GPUI 0.2.2 hack, marked as a shortcut to recheck on a GPUI upgrade. The first attempt, splitting runs so glyphs that landed in the colour font took `Noto Emoji` and shaping again, was dropped: naming `Noto Emoji` deletes it the same way, and it needed a second shape per emoji line and a helper at every call site.
- **Terminal cells unchanged.** The core already makes a wide char a two-cell run of its own, and the frontend places each run at its column, so the emoji's glyph fills its two cells and the rest of the row stays on the grid.

## Risks / Trade-offs

- [Emoji are monochrome on Fedora] → only a COLRv1 rasterizer in GPUI fixes that; recorded in `docs/editor.md`.
- [Which font draws an emoji is cosmic-text's database order] → any font with the glyph draws it; Noto Emoji covers ZWJ sequences and flags.
- [No monochrome emoji font installed: still blank] → install `google-noto-emoji-fonts`; noted in the docs.
- [A GPUI upgrade drops the `m` removal] → the font stays and emoji go blank again; the shortcut comment names the check.
