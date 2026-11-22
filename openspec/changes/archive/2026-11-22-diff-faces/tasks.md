# Tasks

## 1. Diff window

- [x] 1.1 `DiffRow::sign` and `Line::sign`; diff rows draw their face's background on every row and their sign on the first
- [x] 1.2 Removed rows: `-` in `diff-removed`, no label, syntax faces alone over the background
- [x] 1.3 Added lines: `diff-added` background on every row and `+` on the first, replacing the bar
- [x] 1.4 The sign column: wider gutter and `Gutter::signs` in the diff window; the frontend draws it
- [x] 1.5 Mode line: `change` before point's position
- [x] 1.6 Key hints in the diff window, `C-c n/p` for a shared prefix
- [x] 1.7 Bundled theme: backgrounds for `diff-added` and `diff-removed`

- [x] 1.8 Review: the `Staged` index buffer is numbered and its mode line names the branch
- [x] 1.9 Review: point goes to the first change when a file's diff first arrives

## 2. List

- [x] 2.1 `▾`/`▸` on roots with files, `diff-toggle` on `TAB`, folded roots kept across refresh
- [x] 2.2 Directory part in `mode-line-dim`; paths cut from the left with `…` to fit; never wrapped
- [x] 2.3 `hl-line` on the list's point line in an unselected window

## 3. Tests

- [x] 3.1 Core tests for each row piece's face: signs, backgrounds, labels, list faces, fold, fit, hl-line, mode line order, hints
- [x] 3.2 Frontend test: the sign column's position

## 4. Docs

- [x] 4.1 `docs/editor.md`: the view's look as built
