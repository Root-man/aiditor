# Proposal

## Why

Echo-area messages vanish on the next key, and `tracing` output only reaches stderr, which a GUI app launched from the dock never shows. Issue #47 asks for Emacs's `*Messages*` buffer so both can be read back inside the editor. The `*scratch*` buffer it also asks for already exists (text-buffers spec).

## What Changes

- A read-only `*Messages*` buffer that every echo-area message is appended to, one per line.
- The app's `tracing` warnings and errors (formatted as on stderr, without colour codes) are appended to `*Messages*` too; info and below stay on stderr only.
- `*Messages*` is made when the first line arrives, and made again if killed.

## Capabilities

### New Capabilities
- `messages-buffer`: the `*Messages*` buffer: what is logged to it and how it behaves.

### Modified Capabilities

## Impact

- `aiditor-core`: a log entry point for the app and the echo-area paths.
- `aiditor` (frontend): a second `tracing` layer writing into the services channel, which is now made in `main` before tracing starts.
- The open question of where `*Messages*` lives once workspaces exist stays open; until then there is one for the app.
