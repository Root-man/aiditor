# Tasks

## 1. Boxed faces

- [x] 1.1 Add `Face.boxed` (`box` in TOML), merge it in `Face::over`, give the bundled `attachment` a colour; verify with a settings test and a display test that a box reaches its span and survives the region
- [x] 1.2 Return boxed pieces from `line_runs`, strip their run background, keep them on `Row` with shaped x bounds, and paint them after the row's backgrounds; verify with `VisualTestContext` tests for width kept, shared outline, different colours, and marked text

## 2. Right-aligned text

- [x] 2.1 Add `Prop::AlignRight`; pad in `build` with the per-char face, count the rest of the line through folds, fall back to one space; verify with display tests for truncating width, fold, face, fallback, unknown width, and cursor
- [x] 2.2 Add `Line.aligned` so `wrap_lines` keeps a padded line whole, and count `Wrap.from`; verify with display tests for wrapped windows, a too-long line wrapping as without the property, and a line starting mid-wrap
- [x] 2.3 Verify with a harness test that a resize moves the text and that search and kill see one space

## 3. Docs and checks

- [x] 3.1 Record both in `docs/editor.md`'s session display model; verify the text names the property, the fallback and `box`
- [x] 3.2 Draw both in the running app with a throwaway patch, judge a screenshot, and remove the patch; verify `git diff` holds no trace of it
