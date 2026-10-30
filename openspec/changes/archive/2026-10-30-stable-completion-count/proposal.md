# Proposal

## Why

The match count before the prompt (`1/100 `) changes width as matches arrive or the pattern changes, which shifts the prompt and the typed text sideways, most visibly in project grep as batches stream in (#336).

## What Changes

- The count is padded to a stable width: right-aligned in at least 6 columns, then a space, as the approved design's frame 3 draws `  3/48 `.
- A count wider than that widens the field for the rest of that minibuffer; it never narrows while the minibuffer is open, as the completion list's rows only grow. The next minibuffer starts at 6 again.
- Selection and scroll while grep batches stream in already hold; that behavior is written into the spec and given a regression test. Nothing changes there.

Out of scope: find-file's `*/16` count, the count's face, grouped grep rows and the searching state.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `minibuffer`: the count's shown form changes, and it keeps its width while one minibuffer is open.
- `project-grep`: a batch of hits added to the running search leaves the selection and the shown rows in place.

## Impact

`crates/aiditor-core/src/minibuffer.rs` (`Core::minibuffer`, `Completion`) and the tests that assert exact count strings there and in `swiper.rs`. The frontend draws the string it is given, so `crates/aiditor` changes only in the echo line test's expected string. `docs/editor.md` records the padding.
