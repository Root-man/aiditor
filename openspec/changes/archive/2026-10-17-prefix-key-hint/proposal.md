# Proposal

## Why

After a prefix key such as `C-x` the editor waits silently, so a user who forgot what follows has no way to find out. Issue #25 asks for a which-key style hint after a short delay, as part of the display model.

## What Changes

- When a key leaves a prefix pending and no further key arrives within 1 s, the core shows a hint: the pending keys and every key that can follow them, each with the command it runs or `+prefix` for a further prefix.
- The hint follows keymap layering: a key shows what pressing it would actually run.
- The next key, `C-g` included, hides the hint.
- The display model exposes the hint so the frontend can draw it; the skeleton frontend draws it as one line above the message line.

## Capabilities

### New Capabilities

### Modified Capabilities
- `keymaps`: adds a requirement for the hint shown after a pending prefix.

## Impact

- `crates/aiditor-core`: the key loop (`lib.rs`) starts and cancels a hint timer; `display.rs` gains the hint model; `keymap.rs` lists a prefix's bindings.
- `crates/aiditor`: the skeleton frontend draws the hint line.
- `docs/editor.md`: records the decision under Commands and keymaps.
