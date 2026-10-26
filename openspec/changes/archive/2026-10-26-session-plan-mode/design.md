## Context

Tool calls (#294) already rewrite a block in place: a `Stay` marker at its start, its length in chars, `buffer::diff` to replace only changed lines through `Buffer::replace_output`, and properties re-added in sorted order so `properties_near` keeps bisecting. The mode line is built in `display::build` from the buffer alone; it already appends ` [copy]`/` [char]` for terminals.

## Goals / Non-Goals

**Goals:** a plan block that the latest `plan` rewrites in place; the current mode on the session buffer's mode line.

**Non-Goals:** cycling or setting modes, mode names (the id is shown, as `session ready (modes: ...)` lists ids), plan entries as key-bound buttons, a plan pinned near the prompt.

## Decisions

- **One plan block per session, rewritten where it is.** `Session::plan: Option<(MarkerId, usize)>` holds the block's start marker and length. ACP sends the whole list each time and the client replaces it, so the issue's "in place" is literal: the block stays where the first plan arrived, like a tool call. Alternative, moving it above the prompt each time, loses its place in the turn and churns the transcript.
- **Share the in-place rewrite with tool calls.** The diff-and-replace loop in `show_tool` moves into one helper both use, so point on an unchanged line stays put for plans too.
- **Text.** `Plan`, then per entry the content's first line (` …` if it has more, as tool titles) and ` (<status>, <priority>)` in `dim`, the ACP names (`in_progress`, `high`). A one-char `dim` rule at each line's start, as open tool content; entry text is plain, not markdown, so properties never nest. An empty plan is the `Plan` line alone.
- **Mode on the buffer.** `Buffer::agent_mode: Option<String>`, set from `NewSessionResponse.modes.current_mode_id` when ready and from each `current_mode_update`; `display::build` appends ` [<mode>]` after the name, where terminals put theirs. Display needs no access to sessions.

## Risks / Trade-offs

- [A plan block far up the transcript after many turns] → updates still land there, out of view; pinning or moving the plan can come with the post-MVP block rows.
- [The mode id, not its display name] → ids are short and are what config (`default_mode`) uses.
