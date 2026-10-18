# Design

## Context

#50's recovery files are `<pid>-<buffer number>` in `recovery/` under the data dir: a JSON header line (`name`, `path`, `hash`, `time`) then the text. One service thread writes and deletes them. The command line file is read synchronously in `main` before the window opens; prompts with `Yes`/`No` completion already exist for `save-buffers-kill-terminal`.

## Decisions

- **Read at launch, synchronously.** `recovery::list(dir)` reads every file without an extension (a `.tmp` is a write cut short), oldest first, and also reads each visited file as it is now, so the core can visit it with its real line ending and BOM. Unreadable recovery files are logged and skipped. `main` sends the list to the core as `Msg::Recover` after the command line file is opened. Alternative: an async read through `Effect::Open` on `Yes`, which needs pending state in the core for no gain at launch.
- **One prompt per file, not a list buffer.** The `recover-buffers` command keeps the files in the core and asks about the first; its answer is handled and the next one asked, as `save-buffers-kill-terminal` does. `C-g` leaves the rest in the core and on disk. A list buffer would need a new major mode and keys; the prompts do the job.
- **Recovering.** A file is visited (switching to a buffer already visiting it), its text replaced by the recovered text as one undoable edit, and its disk hash set to the recorded one, so the existing save check refuses a file changed since. A buffer without a file reuses the buffer of that name unless it is modified, else gets a new uniquely named one. Then `Msg::AutoSave` runs at once and `Effect::DeleteRecovery(path)` follows it on the same service thread, so the old file goes only after the new one is written. A buffer that does not auto-save (it opened read-only) keeps the old file.
- **Same pid.** A new session may get the pid of the crashed one, so its own files share names with the old ones. The service never deletes a file named with its own pid through `DeleteRecovery`; the session's normal sweep handles those.

## Risks / Trade-offs

- [A second instance still running] Its files are offered as if crashed; `No` deletes them while it runs → it writes them again after its next edit. Recorded as a shortcut.
- [Prompt shows no time] Two files of one path from two crashes look alike → both are offered in age order; the second replaces the first's text, which stays in undo.
- [Same-pid reuse] The session may overwrite an old file of the same name with its own auto-save before the user answers → the old text is already held in memory; lost only if this session also crashes first.
