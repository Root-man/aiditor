# Proposal

## Why

#50 writes unsaved edits to recovery files, but nothing reads them back, so a crash still loses the edits in practice. Issue #51 offers them on the next launch.

## What Changes

- At launch the app reads the recovery dir. Each file left by an earlier session (a normal quit deletes its own, so these are from crashes) is offered in turn in the minibuffer: `Recover auto-saved <path>? ` with `Yes`/`No` completion, `Yes` first.
- `Yes` opens the recovered text as a modified buffer: a file's buffer visits its original path (reusing a buffer already visiting it) and keeps the disk hash the text was based on, so a save refuses to overwrite a file changed since; a scratch buffer's text goes into the buffer of that name, or a new one if that one is already modified. The text is auto-saved at once under this session, then the old file is deleted.
- `No` deletes the recovery file. `C-g` keeps the rest for the next launch, and `M-x recover-buffers` offers them again.
- New `Msg::Recover`, `Effect::DeleteRecovery`, `recovery::list`, command `recover-buffers`.

Not in this change: comparing with the disk (diff buffer, M1), a list buffer showing all files at once, the auto-save time in the prompt, skipping files of another instance still running.

## Capabilities

### New Capabilities

None.

### Modified Capabilities
- `auto-save`: adds recovering the files on launch.

## Impact

- `crates/aiditor-core`: `lib.rs` (message, effect, command, recovered list), harness.
- `crates/aiditor-services`: `recovery.rs` (`list`, delete by path).
- `crates/aiditor`: `main.rs` lists the files at launch and routes the new effect.
- `docs/editor.md`: Crash safety records the decisions.
- No new dependencies.
