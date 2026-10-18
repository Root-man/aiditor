# Design

## Context

Echo-area messages are set by assigning `Core::message` from about twenty places, plus `Core::report`, which joins messages from one load with `; `. Tracing is set up in `main` with one `fmt` subscriber to stderr. Services send `Msg`s to the core on one bounded Tokio channel made in `Editor::new`.

## Goals / Non-Goals

**Goals:** echo messages and log lines in `*Messages*`, with no new dependency.

**Non-Goals:** `messages-buffer-mode` keys, point following new lines, a size cap, per-workspace buffers.

## Decisions

- **One `Core::echo` for every message.** All `message = Some(..)` sites call it; it sets the echo area and appends the line. Hooking `handle` to diff the message before and after would miss a repeat like two `Quit`s. `report` logs each part on its own line.
- **`Msg::Log(String)`** appends a line from the app. The core stays free of `tracing-subscriber`.
- **Tracing through `fmt`'s writer.** A second `fmt::layer()` with ANSI off and its own `WARN` level filter (info lines are noise in an editor buffer; stderr keeps them) writes into a small `io::Write` that `try_send`s `Msg::Log` on the services channel. `fmt` writes one event per write, so no own formatter is needed. The channel is made in `main` before tracing starts, so lines logged before the window opens wait in it. `try_send`, never a blocking send: events are logged on the UI thread too.
- **Append bypasses read-only and undo.** `Buffer::append` inserts at the end without recording a change, with read-only lifted for that insert.
- **Made lazily.** Looking the buffer up by name on each line lets a killed `*Messages*` come back, and keeps the startup buffer list (and every buffer-list test) as it was.

## Risks / Trade-offs

- [The channel is full] → the log line is dropped (`shortcut:` comment); 256 slots are plenty for M0.
- [Unbounded growth] → none in M0; a cap can come with long sessions.
- [`RUST_LOG` above warn] → the global filter gates both layers, so it hides warnings from `*Messages*` too; intended, one level knob.
- [A log line logged while handling `Msg::Log`] → would loop; handling it logs nothing.
