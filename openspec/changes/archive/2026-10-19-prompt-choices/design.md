# Design

## Context

See proposal.md for why. Locations below are as of commit `8fa1ddd`; `split-core` moves the minibuffer code into its own module, so each is named by item as well as by line.

- `Ctx::read_string(command, args, prompt)` (`crates/aiditor-core/src/lib.rs:297`) builds the candidate list from `command.spec.completion` (`lib.rs:304-307`) and each candidate's note from `command.spec.annotation` (`lib.rs:317`), then stores a `Minibuffer` (struct at `lib.rs:148-161`).
- Its callers: `Ctx::try_call` for declared prompts (`lib.rs:273`), `Ctx::kill_buffer` (`lib.rs:1236`), `Ctx::save_buffers_kill_terminal` (`lib.rs:1273`, `lib.rs:1282`) and `Ctx::recover_buffers` (`lib.rs:1353`).
- `Ctx::exit_minibuffer` (`lib.rs:359`) pushes every non-empty, non-repeated answer onto `core.history[prompt]` (`lib.rs:373-377`).
- The guessing completions are the builtin `ArgSpec`s set in `impl Default for Core` (`Core::default`): `save-buffers-kill-terminal` (`lib.rs:1730-1740`, `len() % 2`), `recover-buffers` (`lib.rs:1741-1745`) and `kill-buffer` (`lib.rs:1746-1753`, `is_empty()`).

## Goals / Non-Goals

**Goals:** a handler names the candidates of the question it asks; those answers skip history; no guessing left in the builtin specs.

**Non-Goals:** a new public API, a shared `yes-or-no-p` history, single-key answers, recursive minibuffers (see proposal for the full out-of-scope list).

## Decisions

- **Add a `choices: Option<&[&str]>` parameter to `read_string` rather than a sibling `ask`.** There are four callers; `try_call` passes `None`, the three handlers pass `Some(&["Yes", "No"])` and the like. A sibling would duplicate or wrap the whole function for one branch. `&[&str]` keeps the call sites to a literal; `read_string` turns it into the `Vec<String>` the injector loop already takes.
- **The choices replace only the candidates.** `candidates = choices.map(to_vec).or_else(|| spec.completion(...))`. Annotations still come from `spec.annotation` (none of these commands has one), so nothing else in the completion setup changes. A prompt with choices is a completing prompt, so `minibuffer-completion` turns on and fuzzy matching, `RET` on the first candidate and case-insensitive matching (`y`, `no`) behave exactly as today.
- **`history: bool` on `Minibuffer`, set to `choices.is_none()`; `exit_minibuffer` records only when it is true.** Not recording also means `history_element` finds no entry for those prompts and reports `Beginning of history; no preceding item`, with no change there. Alternative rejected: deriving it from "the prompt is not in `spec.prompts`", which ties history to prompt text again.
- **Remove `completion` from `save-buffers-kill-terminal` and `recover-buffers`, and make `kill-buffer`'s `|core, _| core.buffer_names(true)`.** Their candidate order moves verbatim into the handlers: `Save file` `Yes, No`; exit `No, Yes`; recover `Yes, No`; kill confirm `no, yes`.
- **Spec home: `minibuffer`.** The candidate list and history are minibuffer requirements; `commands` only covers declared prompts, which do not change.

## Risks / Trade-offs

- [Candidate order or case drifts while moving into the handlers] → the existing harness tests assert `listed(...)` order for kill, quit and exit prompts, and the `quitting`, `buffer-switching` and `auto-save` scenarios are unchanged; they must pass untouched.
- [Rebasing over `split-core`] → tasks name functions, not lines; the change is small enough to redo by hand.
