# Design

## Sign column

Frame 15's gutter is 10px, the line number right-aligned, 8px, a one-cell sign, 10px, then the text; a plain gutter is 10px, number, 12px. A window showing the diff or holding diff rows gets the wider gutter (`display::sign_width(cell)` more), with blank labels when line numbers are off, and `Gutter::signs` holds a sign and its face per row, empty in every other window, so nothing else changes. The frontend shifts the label left by the sign's room and draws the sign beside the text.

The sign belongs to the row, so `display::Line` gets `sign: Option<(char, FaceName)>`, set on the first row of an added line or a removed diff row only, so a wrapped row isn't counted twice. `DiffRow` gets `sign: Option<(char, FaceName)>` too; its rows take the face's background as a `Background` decoration, the sign the face's foreground. Added lines get the same from `frame`, which knows the added ranges. `hl-line` sits under both, as frame 15's current added line keeps the added background.

Removed rows lose their label (frame 15 numbers only the new side) and the `diff-removed` face under their text: the background carries the colour, so their text is plain where the old side has no syntax face, as the frame draws it.

## List

`TAB` (`diff-toggle`) in the list folds or unfolds the root of the row at point and moves point to its heading; the folded roots are kept on the list, so a refresh keeps them. A root with files has `▾` or `▸` in the two columns before its name; a clean, non-git or failed root keeps them blank, as frame 15's `notes`. Folding a root whose file is shown leaves point on the heading, which leaves the shown diff, as any non-file row does.

Paths are cut at display time, since the cut depends on the window: the list is never wrapped, and a file row longer than the window loses chars from the start of its path, which then starts with `…`. The directory part is in `mode-line-dim`, the frame's `dim`.

`hl-line` is put on the list's point line in any window showing it, not only the selected one, so the shown file's row stays marked while the diff window is selected.

## Mode line and hints

The counts `+<added> −<removed>` move to `status-running` and `status-failed`, as a tool call's are, since the diff faces now carry backgrounds the bar would show. `change <i>/<n>` goes before `<line>:<col>` instead of first on the right. Key hints show when the current buffer is the list or the diff window's buffer while it shows the diff; a pair of keys sharing a prefix reads `C-c n/p`. Hints for commands not built yet (stage, discard, side by side) are left out, as hints for unbound commands are.

## Not done

The frame drops the language segment from the mode line; it stays, as every file buffer's mode line has it. The user's own Gruvbox themes lack `diff-file-*`, `diff-list-group` and `session-list-count`, which then come from the bundled defaults (dark values) in the light theme; that is the user's settings file, not the repository's.
