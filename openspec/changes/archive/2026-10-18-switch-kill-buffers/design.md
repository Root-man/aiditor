# Design

## Context

`Buffers` is a `BTreeMap` by id; `Core::open` makes a new buffer and `Window::show` swaps the selected window onto it, dropping the old window's markers. A window's point is a marker in its buffer. Prompts come from `ArgSpec::prompts`, completion from `ArgSpec::completion`, and the minibuffer is closed before its command runs again with the answer.

## Goals / Non-Goals

**Goals:** the three commands, recency order, a remembered point per buffer, safe kills.

**Non-Goals:** per-window buffer history (`window-prev-buffers`), preferring buffers not shown in any window, raising a buffer when a window is selected, reusing the buffer of an already visited file (find-file's job), workspace scoping (M1), killing a buffer's agent or terminal.

## Decisions

- **Recency lives in `Buffers`** as a `Vec<BufferId>`, most recent first: `add` puts a new buffer last (as Emacs's `buffer-list`), `kill` drops it, `raise` moves one to the front. Keeping it beside the map keeps them in step. Alternative: a use counter per buffer, sorted on read; same size, but a list is the order itself.
- **A buffer's point is a marker it owns**, made at 0 with the buffer. `Window::release` (every way a window stops showing a buffer: show, delete, delete others) sets it to the window's point; `Window::new` starts at it. One place records, one place reads. Alternative: a per-window history like Emacs; more state for no gain at this stage.
- **Switching** goes through one `Core` helper that raises the buffer and shows it unless it is already shown; `Core::open` uses it, so visiting raises too.
- **`C-x 4 b`** splits below when the tree has one leaf, then selects the next window with `other-window`'s logic, then switches.
- **The kill confirmation** is a second prompt the handler opens itself only when needed, through the same `read_string` path; the spec's completion function returns `["no", "yes"]` once one string is in, so an empty `RET` picks `no`. The prompt is one static string without the buffer name, since prompts are `&'static str` and key the history.
- **Replacement after a kill** is the most recent listed buffer other than the killed one, else a new `*scratch*`; every window showing the killed buffer, selected or not, shows it before the buffer goes.
- **Unique names** are made in `Buffers::add` by trying `<2>`, `<3>`, ... until free.

## Risks / Trade-offs

- [Selecting a window does not raise its buffer, so `C-x b RET` after `C-x o` may not pick the other window's buffer] → matches the issue's "most recent first" by switching; raise on select when it bites.
- [`yes` is a completion candidate, so `y RET` confirms] → still a deliberate answer; the default (`RET` alone) declines.
