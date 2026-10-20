# Tasks

## 1. Grow-only completion list

- [x] 1.1 Keep the most rows shown in the per-prompt completion state and pad the minibuffer display's candidates and annotations with empty rows to it; verify with a core test for both spec scenarios (shrinking keeps 10 rows, the next minibuffer starts at its own count) and a GPUI test that the window area keeps its height as the list shrinks
- [x] 1.2 Record the behavior in the `docs/editor.md` minibuffer paragraph and verify it names #160
