# Tasks

## 1. Core syntax and result display

- [x] 1.1 Snapshot current source syntax with Swiper text and expose byte-ranged syntax faces for visible result rows; verify source colours, multiline and injected context, locals, Unicode, pending edits, source changes and plain buffers through core tests.
- [x] 1.2 Carry syntax faces through the minibuffer display while preserving filtering, scrolling and theme reloads; verify those behaviours through harness tests.

## 2. Frontend and documentation

- [x] 2.1 Render candidate syntax through the existing text-run helper, retaining selection backgrounds; verify colours, font styles, byte lengths and selected/unselected backgrounds with a GPUI test and existing completion tests.
- [x] 2.2 Document Swiper syntax snapshots and theme behaviour in docs/editor.md; validate the line-search delta with openspec validate --strict.

## 3. Integration verification

- [x] 3.1 Run cargo fmt, workspace clippy and tests, dependency/panic-profile checks and benchmark reporting tests; resolve failures according to the repository's flake policy.
- [x] 3.2 Validate result-row layout and source-face rendering through GPUI simulated input and text shaping; record any remaining native visual/input checks for user review.

## Native checks

The GPUI test covers simulated keyboard input, result-row bounds, source-face text runs and shaping, theme reloads and selection backgrounds. On 2026-11-01 the user confirmed the native check passed: C-s searching for `needle` retained comment and string colours, C-n/C-p moved the selection background without losing syntax colours, and C-g restored the source position. Native checks were performed by the user because this session's computer-use tool exposes browser surfaces only.

The reviewed command was `cargo run -p aiditor -- --config-dir /private/tmp/aiditor-402-review/config /private/tmp/aiditor-402-review/review.rs`, using prepared scratch fixtures. Markdown fence context is covered by the automated core tests.
