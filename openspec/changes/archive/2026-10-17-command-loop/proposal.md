# Proposal

## Why

Keys already resolve through the keymaps and run commands, but the loop forgets what ran: a command cannot tell whether it follows itself (as `C-k` appending kills or repeated `C-l` will need), and nothing keeps the keys for keyboard macros (M5). Issue #22 finishes the loop so those features attach to it without changing it.

## What Changes

- The core tracks the command the current key sequence runs (this command) and the one the previous complete sequence ran (last command). Prefix keys leave both alone; a sequence that runs no command (a plain key that types or moves, an undefined sequence) leaves no last command; `C-g` quitting a prefix counts as `keyboard-quit`. Commands called by name from inside a command do not change them.
- The core can record the key stream: once recording starts, every key the loop receives is kept until the recorded keys are taken, which also stops recording.
- The undo boundary per command stays as it is (one group per command, typed chars amalgamate).

Not in this change: keyboard macro commands, `recent-keys`, `Msg::Insert` recording, prefix arguments.

## Capabilities

### New Capabilities
- `command-loop`: what the loop remembers about each key sequence: this and last command, and the recorded key stream.

### Modified Capabilities
None. Key lookup, undo grouping and command calls keep their specified behavior.

## Impact

`aiditor-core` only: `Core` gains the command tracking and recording fields with public accessors, `Ctx::key` sets them. The frontend is unchanged.
