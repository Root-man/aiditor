# Design

## Context

The core is a plain state machine with fake-clock timers (`Timers`), and services run off the main thread, answering with messages. Saves already go `Effect::Save` → file service → `Msg::Saved`. Ropes are cheap to clone.

## Decisions

- **Two shared timers, not one per buffer.** Every key or text insert while some covered buffer has unwritten edits restarts a 2 s idle timer and starts a 30 s timer if none is running; both deliver `Msg::AutoSave`, which cancels both and emits `Effect::AutoSave` for each covered, modified buffer whose tick differs from the tick last written. So typing nonstop still writes every 30 s, and an idle editor never wakes.
- **Covered buffers** are those not read-only and not hidden by a leading space: file buffers and scratch-like buffers, but not `*Messages*` or the minibuffer.
- **One sweep removes files.** After each message the core emits `Effect::DeleteAutoSave` for every buffer with a recovery file that is gone or unmodified: save, kill and a future revert all land here. When the effects include `Effect::Quit`, every recovery file is deleted, placed before the `Quit` so the app runs them first.
- **One service thread.** The app sends both effects, in order, on a std channel to one thread, so a delete never overtakes the write before it. On `Effect::Quit` the app closes the channel and joins the thread before quitting. Failures are logged as warnings, so they reach `*Messages*`.
- **File format.** Named `<pid>-<buffer number>` so sessions never collide; written as `<name>.tmp`, synced, renamed. First line is JSON `{"name","path","hash","time"}` (`time` in Unix seconds), then the text with LF line endings, as the buffer holds it. The dir is created with mode 0700, as the files hold unsaved text.
- **Fixed hash.** `files::hash` becomes FNV-1a 64, a few lines, stable across Rust releases.
- **Recovery dir from the platform.** `parse_args` fills it from `directories::ProjectDirs::data_dir()`, like the config dir, so tests that build `Args::default()` never write to the real data dir.

## Risks

- A path that isn't UTF-8 is stored as `null`, so its recovery file only names the buffer.
- A crash during the 2 s idle window loses those last seconds of edits.
- A failed recovery write is only logged; the buffer is written again after its next edit.
- Quitting waits for queued recovery writes; each is one small file.
