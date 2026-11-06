# Design

## Context

Scoped buffers get a `<workspace>` suffix when they are made: `vterm<n><ws>` (`terminal.rs`), `*scratch*<ws>` (`buffer::scratch_name`), `*<agent> stderr*<ws>` and `*<agent> output: …*<ws>` (`agents.rs`). Buffer names are unique across all workspaces (`buffer::unique`), and the suffix keeps two workspaces' `vterm1` apart. `display::mode_line` and `display::terminal_mode_line` show `buffer.name` verbatim. `window_display` has no workspace name, only the project's `roots`.

The workspace list already hides the suffix: `workspace_list.rs` strips `<{workspace.name}>` with `strip_suffix`.

## Goals / Non-Goals

**Goals:**
- Hide the current workspace's suffix on the buffer mode line, the same way the workspace list hides it.

**Non-Goals:**
- Changing buffer names, uniqueness, `switch-to-buffer` candidates or persistence (option B).
- Renaming suffixed buffers on `rename-workspace` (option C). Today only the scratch follows a rename, so after one `vterm1<old>` shows its stale suffix in full. That is a separate fix.
- Agent session and file mode lines, which never show `buffer.name`.

## Decisions

- **Strip the current workspace's name in `window_display`, after the line is built.** A window only shows buffers of the current workspace or of none, so the current name is the only suffix that can apply. `window_display` (`frame.rs`) gets `workspace: Option<&str>` beside `roots`. For a buffer with no file, it trims the suffix from the last span of the mode line's first segment, which holds the name in both the plain and the terminal line. `frame.rs` passes the current workspace (`None` in the lobby), and the minibuffer's one-line display passes `None`. Session mode lines replace the whole line afterwards, so they are unaffected.
  - *Alternative:* pass the name through `display::build` into `mode_line` and `terminal_mode_line`. Rejected: `build` has about 18 test callers that would all need an extra `None`, for the same behaviour.
  - *Alternative:* store a display name on `Buffer`. Rejected: it adds state that a rename would have to keep in sync.
- **Exact `strip_suffix`, as the workspace list does.** `*claude stderr*<work><2>` is left whole, matching the list, rather than becoming `*claude stderr*<2>`. One rule in both places beats handling a rare duplicate.
- **Match against the suffix with `^J` for newlines,** as the mode line shows the name, so a newline in a workspace name still matches.

## Risks / Trade-offs

- [A user-named buffer that happens to end in `<current-ws>` loses that text on its mode line] → It's harmless: the full name still shows in `switch-to-buffer`, and the workspace list already behaves the same way.
- [The stale suffix after a rename stays visible] → This is out of scope (see Non-Goals). It's arguably useful, since it shows the buffer is from before the rename.
