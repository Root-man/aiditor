# Design

## Context

`Ctx::key` in `crates/aiditor-core/src/lib.rs` already is the loop: it sets the undo boundary, pushes the key onto the pending sequence, looks it up through the layered keymaps and calls the bound command, falling back to the hardcoded editing keys. This change adds what it remembers; see proposal.md.

## Goals / Non-Goals

**Goals:** this/last command and a recordable key stream, both kept by the existing loop.

**Non-Goals:** macro commands and playback, a `recent-keys` ring, letting a command overwrite this command (Emacs's `setq this-command`); add that when kill appending needs it.

## Decisions

- **Names, not ids.** `this_command` and `last_command` are `Option<&'static str>`, the same type the keymaps and registry already use for command names.
- **Set in `Ctx::key`, not `Ctx::call`.** Only a complete key sequence changes them, so nested calls and harness calls by name leave them alone. `last_command = this_command.take()` once the sequence ends; prefix keys return before that.
- **Fallback edits count as no command.** The hardcoded plain keys are not registered commands yet; the insertion and motion commands issue will name them. Until then they leave `None`.
- **Recording is `Option<Vec<Key>>`.** `Some` means recording; the key is pushed first thing in `Ctx::key`, before C-g handling, so playback sees exactly what came in. Alternative, recording `Msg`: no other input message exists yet, and `Msg::Insert` will add its own push when it lands.

## Risks / Trade-offs

- [Unbounded recording if never taken] → only macro commands start it, and they stop it; M5 adds a length cap if needed.
