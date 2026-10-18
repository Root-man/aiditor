# Design

## Context

Completion (#39) feeds `Vec<String>` candidates from `ArgSpec::completion` into one shared `nucleo` matcher when the minibuffer opens; the display model lists up to 10 matched candidates. Keymap layering is `Core::layers()`, which uses the current buffer, and the current buffer is the minibuffer once it opens.

## Goals / Non-Goals

**Goals:** run any command by name, show its key, keep annotations generic for later prompts (buffers, files).

**Non-Goals:** Emacs's "You can run the command with ..." hint, `M-X` (mode commands), a predicate hiding minibuffer-only commands, several keys per command, history-based ordering of candidates.

## Decisions

- **Annotation hook in `ArgSpec`**: `annotation: Option<fn(&Core, &str) -> Option<String>>`, called for each candidate in `read_string` before the minibuffer buffer is made current, so the keys are the calling buffer's. Alternative: change `completion` to return pairs, which would touch every caller and conflate matching with display. The matcher item becomes `(candidate, annotation)`; column 0 stays the candidate, so annotations are never matched.
- **Display**: `display::Minibuffer` gets `annotations: Vec<String>`, parallel to `candidates`, empty string for none. Keeps existing `candidates` tests and snapshots intact apart from the new field.
- **Where-is**: a `Core::where_is(name)` walks every layer's trie, keeps sequences whose full `lookup` resolves to `name` (so shadowed keys drop out), and picks the shortest, ties by `kbd` text. About 50 commands times about 80 bindings is trivial at open time.
- **Running**: `execute-extended-command` has prompt `M-x `, completion over the sorted registry names, and a handler that sets `this_command` to the chosen name and `ctx.call`s it with `args.prefix`. `call` already reports `No command named X`.
- **Frontend**: the candidate row draws the annotation after the name, separated by two spaces, dimmed with 0.6 opacity, as the theme has no dim colour.

## Risks / Trade-offs

- A command run by name that relies on `this_keys` (e.g. `self-insert-command`, `digit-argument`) fails with its own message, as in Emacs. Acceptable.
- Annotations are computed once per open; a keymap reload while `M-x` is open is not reflected until it reopens.
