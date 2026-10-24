# Tasks

## 1. Emoji fallback

- [x] 1.1 Add `emoji.rs`: the table check of a font file, unit-tested on synthetic table directories (COLRv1-only undrawable; CBDT, sbix, COLR v0, plain outline and collection files drawable; a truncated file an error) and on Fedora's font when present
- [x] 1.2 At start, drop `Noto Color Emoji` from GPUI's font database when it can't be drawn; `cargo test -p aiditor` passes

## 2. Terminal and docs

- [x] 2.1 Add the emoji scenario to the core's wide-char test; `cargo test -p aiditor-core` passes
- [x] 2.2 Record in `docs/editor.md` why GPUI can't draw COLRv1 and the fallback
- [x] 2.3 Check on Linux in the running app that 🦀 and other emoji show in a file buffer (screenshot); the terminal is left to the review
