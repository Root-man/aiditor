# Proposal

## Why

Principle 2 says every feature is a named command reachable from keys and `M-x`. The key trie, command loop, prefix argument and `M-x` all need a place to look commands up by name and a uniform way to hand them their arguments, so the registry comes first.

## What Changes

- A registry of named commands in the core. Each command has a name, an argument spec (whether it takes the region, and the strings it prompts for) and a handler that gets the context and its arguments and returns success or an error message.
- Every call carries the prefix argument (absent or a number), as the design says it goes to every command, so the spec does not declare it.
- Calling a command by name with arguments: an unknown name, a region the command needs but cannot get (no mark), or missing prompted strings fail before the handler runs. A command that needs the region and is not given one gets point to mark of the current window.
- Errors, from the call or the handler, go to the message line (the echo area stand-in until #36).
- The hardcoded undo, redo, save and settings-reload keys become registered commands (`undo`, `undo-redo`, `save-buffer`, `reload-settings`) that the key handler calls by name. Key behavior does not change.
- Out of scope: the key trie and keymaps (#21), the command loop, parsing the prefix argument from keys, prompting through the minibuffer, `M-x` and listing commands for completion.

## Capabilities

### New Capabilities
- `commands`: named commands, their argument specs, calling them by name and how errors are reported.

### Modified Capabilities

## Impact

- `crates/aiditor-core/src/command.rs` (new): command, argument spec and arguments types, and the registry.
- `crates/aiditor-core/src/lib.rs`: the core owns the registry with the built-in commands; `Ctx::call` runs a command by name; the key handler calls the built-ins.
- No new dependencies; the frontend is unaffected.
