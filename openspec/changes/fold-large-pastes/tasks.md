# Tasks

## 1. Folding

- [x] 1.1 Add paste_fold_lines with default 20 and verify valid overrides and invalid settings with settings tests.
- [x] 1.2 Support display newlines and multiline replacement folds; verify header, cursor and following-text projection with display tests.
- [x] 1.3 Track and paint yanked session ranges, including yank-pop, TAB previews and folded sent echoes; verify thresholds, full sends, streaming and editing with harness tests.

## 2. Integration

- [x] 2.1 Record the four-line preview decision in docs/editor.md and verify it agrees with the specs and frame 6b.
- [x] 2.2 Run fmt, workspace clippy/tests and remaining CI checks; fix failures under the flake policy.
- [x] 2.3 Verify folded and preview painting with GPUI tests and inspect the user screenshot against frame 6b before the local commit and user review.

Visual evidence: the user supplied `Screenshot 2026-11-03 at 14.54.19.png`; inspected on 2026-11-03. It shows an open transcript paste with its chip header, four diagnostic lines and `… 21 more lines`. The screenshot was removed at the user's request. Folded prompt and toggle painting are covered by the GPUI test; full sending and folded sent echoes are covered by the core harness.
