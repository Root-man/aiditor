# Design

## State

The list buffer's `DiffList` gains the right window's id (set when the view opens) and `Option<Shown>`: the list row shown, the sides, the held old text, the new side's buffer, its hunks, the latest request id, the buffer tick the hunks are for, and a pending re-diff timer. A side buffer (index text or the reason) is owned by the view and killed when another file is shown or the view quits. Everything dies with the list.

## Flow

After every message (`Core::handle`, beside `preview`), `sync_diff` looks at the current workspace's list: if the list window's point is on a file row other than the shown one, it starts showing it: a new request id, `FileOp::GitDiff` with the sides. On `GitDiffed`:

- `Err`: a side buffer holding the reason, shown in the diff window.
- `Staged`: a read-only side buffer with the index text; hunks from the answer.
- `Changes`: the old text is kept; the file's buffer is used if open, else `FileOp::Open` with a view-owned request, intercepted in `on_opened` as `client_read` is. Once the buffer exists it is shown and `FileOp::DiffLines` (old, buffer text) asks for the hunks.

`sync_diff` also watches the shown buffer's tick: when it differs from the tick of the last request, it (re)starts a 300 ms timer delivering `Msg::Rediff`; firing sends `DiffLines` with a new request id. Answers with any other id are dropped.

Rows: each hunk's old lines become one-row `DiffRow`s anchored above `new.start` (after the last line when past it), labelled with the old line number, the text in `diff-removed`. Added lines: `Decoration::Bar(diff-added)` on display lines whose number is in a hunk's `new`, applied in `frame()` for the diff window only, beside the mode line segments.

## Commands

`diff-next-change`/`-previous-change` move the diff window's point to a hunk's `new.start` (clamped to the last line). `diff-next-file`/`-previous-file` move the list window's point. `diff-copy-removed` copies `old[hunk.old]` of the hunk whose `new` range contains point's line or starts on it. `diff-visit` (`C-g`) deactivates an active region, else takes the shown buffer and point, quits, and shows it in the selected window (a `Staged` diff visits the file with a destination).

## Decisions

- `C-g` instead of `RET`: `RET` must insert a newline in an editable diff (user's choice).
- Added lines get a bar, not a background, until #508 gives `diff-added` a background.
- Removed rows carry old line numbers as #505 says; frame 15's sign column and blank label are #508's.
- `diff-view` is skipped in the minibuffer so its keys, `C-g` above all, don't shadow the minibuffer's.
