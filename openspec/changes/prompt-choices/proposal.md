# Proposal

## Why

Commands that ask a follow-up question (`kill-buffer`'s confirmation, `save-buffers-kill-terminal`'s `Save file <path>? ` and `Modified buffers exist; exit anyway? `, `recover-buffers`'s `Recover auto-saved <path>? `) get their candidates from the command's one `ArgSpec.completion` function, which has to guess which question is open from how many strings have arrived (`args.strings.len() % 2`, `args.strings.is_empty()`). The handler and the completion function must agree on that encoding and nothing checks it. Their `Yes`/`No` answers also go into a history keyed by the prompt text, so every path makes a new history key for the session and `M-p` there recalls `Yes` or `No`; Emacs's `y-or-n-p` keeps none. M1's file-conflict prompt (Keep mine / Take disk / Compare) is one more such question, so this is worth settling first.

## What Changes

- A command can open the minibuffer for a further question with its own list of candidates. That list replaces the command's declared completion for that prompt, and the answer is not added to any history.
- Prompts a command declares in its spec keep using the spec's completion, annotations and history as now.
- `recover-buffers` and `save-buffers-kill-terminal` drop their `completion` function; `kill-buffer`'s returns buffer names only. Their questions pass their choices directly.
- Answer rules stay exactly as specified: typed `yes`, `y` or `no` pick through fuzzy matching, `RET` alone picks the first candidate, `kill-buffer` lists `no` first, the exit prompt `No` first, `Save file` prompts `Yes` first, and `C-g` keeps the remaining recovery files.
- `docs/editor.md` drops "different candidates per prompt" from the Minibuffer Not yet list and notes the history rule.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `minibuffer`: "Each prompt keeps a history" no longer records answers to a question asked with its own choices; a new requirement lets a command ask a further question with its own candidates, independent of its declared prompts. It goes in `minibuffer` rather than `commands`, because `commands` only says what a command's declared prompts receive, while the candidate list, its order and history are minibuffer behavior already specified there.

Checked and unchanged: `quitting` (`Save file` lists `Yes` then `No`, the exit prompt `No` then `Yes`, matched case-insensitively), `buffer-switching` ("Killing a modified file buffer asks first": `no` then `yes`) and `auto-save` (recovery `Yes`/`No`, `C-g` keeps the rest). Each question keeps its prompt text, candidate order and answer handling, so their scenarios hold as written and need no delta.

## Impact

- `crates/aiditor-core/src/lib.rs`: `Ctx::read_string` and its four callers (`Ctx::try_call`, `Ctx::kill_buffer`, `Ctx::save_buffers_kill_terminal`, `Ctx::recover_buffers`), `Ctx::exit_minibuffer`, the `Minibuffer` struct, and the builtin `ArgSpec`s set in `impl Default for Core` (`Core::default`). After `split-core` most of this lives in the minibuffer module; the names stay.
- `crates/aiditor-core/src/command.rs`: the `ArgSpec.completion` doc comment ("each prompt" now means each declared prompt).
- Tests in `crates/aiditor-core/src/harness.rs`; `closing_the_window_offers_to_save` in `crates/aiditor/src/main.rs` must still pass.
- No public API, dependency or data format change. `read_string` is private to the core.
