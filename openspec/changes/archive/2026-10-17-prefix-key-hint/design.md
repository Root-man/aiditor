# Design

## Context

Keys go through `Ctx::key`, which keeps the pending sequence in `Core::pending` and returns early while it is a prefix. Core timers deliver a `Msg` at a deadline. The display model (`display.rs`) is built per window; the message line lives on `Core`, not in `WindowDisplay`.

## Goals / Non-Goals

**Goals:** a timer-driven hint in the core, testable through the harness on the fake clock.

**Non-Goals:** a setting for the delay, paging long hints, column layout in the frontend (the display-model renderer issue draws it properly).

## Decisions

- **A timer delivering `Msg::Hint`.** Each key cancels the hint timer and clears the hint; a key that leaves a prefix pending starts a new 1 s timer. When it fires the core marks the hint shown. Alternative: compare `now` against a stored instant on every `Tick`; rejected, the app arms its wake-up only from timers.
- **Computed when the display asks.** `Core::hint()` builds the entries from the pending keys and the current keymaps, so it never goes stale. Entries are the union of the bindings under the pending sequence in each layer where it is a prefix, in lookup order, first layer winning per key, so each entry matches what `lookup` would return for that key.
- **Frame-level, not in `WindowDisplay`.** The hint belongs to the echo area, like the message, so it is a `display::Hint { keys, entries }` returned by `Core::hint()`, not a field of each window's display.
- **1 s delay**, Emacs's `echo-keystrokes` and which-key's default, hardcoded as a constant.

## Risks / Trade-offs

- [The skeleton frontend draws the hint on one clipped line] -> long hints are cut off until the display-model renderer lays them out; marked as a shortcut.
