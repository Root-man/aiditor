# Proposal

## Why

Commands can only be run by their keys today, and the minibuffer and its completion list (#38, #39) have no user yet. `M-x` makes every command reachable by name and is the first visible use of the minibuffer.

## What Changes

- `M-x` runs `execute-extended-command`: it reads a command name at the prompt `M-x `, completing over every registered command, and runs the chosen one with the prefix argument `M-x` got.
- Each candidate in the list shows the key that runs that command in the buffer `M-x` was called from.
- A prompt can annotate its candidates; the annotation is shown after the candidate and is not matched.

## Capabilities

### New Capabilities

### Modified Capabilities
- `commands`: commands can be run by name from `M-x`.
- `minibuffer`: completion candidates can carry an annotation shown in the list.

## Impact

- `aiditor-core`: `ArgSpec` gains an annotation hook, the display model's minibuffer gains annotations, a new builtin command and a global `M-x` binding.
- `aiditor` frontend: draws each candidate's annotation after it.
- `docs/editor.md`: records the decisions.
