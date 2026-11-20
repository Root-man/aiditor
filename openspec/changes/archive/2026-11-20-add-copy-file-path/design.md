# Design

## Context

- A file buffer's path is `Buffer::path()` (`buffer.rs`), already absolute; non-file buffers return `None`.
- The core does no filesystem work. `aiditor_services::files::head()` already walks up from a visited file to the nearest `.git` (directory or file) and returns a `Head`, which the core keeps on the buffer's file state (`file.head`) and refreshes when the watcher reports a change (`watching.rs`). `Head` holds the `HEAD` file's path, the branch and `changed`, but not the directory the `.git` was found in; for a linked worktree or submodule `HEAD` lives elsewhere, so the root can't be derived from it.
- `Ctx::kill` (`kill_ring.rs`) pushes text with `kill_new`, records `clipboard_seen` and emits `Effect::SetClipboard`; first it calls `take_clipboard` so foreign clipboard text is saved before being overwritten, which needs the command's `ArgSpec { clipboard: true }`.

## Goals / Non-Goals

**Goals:** two `M-x` commands per `specs/file-paths/spec.md`, sharing the kill ring's clipboard path.

**Non-Goals:** key bindings, a line number suffix (`a.rs:12`), `~`-abbreviation, copying from non-file buffers (terminal cwd, session cwd), asking `git rev-parse` for the root.

## Decisions

- **The repository root rides on `Head`.** Add `root: PathBuf` (the directory holding `.git`) to `Head`, set in `head()` from the `.git` it found. The command reads `file.head` and `strip_prefix`es the root. Alternatives: a new filesystem walk in a services request (async round trip for a synchronous command), or `files::root()` (falls back to the folder, and is a services function the core can't call). `Head` is already computed for every visited file, so this costs one field.
- **No `Head`, no relative path.** When `head` is `None` (outside a repository, or `HEAD` unreadable) the command fails with `File is not in a git repository`. A silent fallback to the project root or the absolute path would paste a path of a different kind than asked for.
- **Kill ring too, not clipboard only.** Emacs's equivalents (`kill-new`) do this, and `C-y` then works in the editor without the clipboard round trip. Factor the tail of `kill` (take foreign clipboard, `kill_new`, `clipboard_seen`, `SetClipboard`) into one `Ctx` helper both use; both commands set `clipboard: true` so `take_clipboard` has the text. The copy does not join a previous kill.
- **Lossy paths.** Paths are copied with `to_string_lossy`; a non-UTF-8 path is rare on macOS and the clipboard is text.

## Risks / Trade-offs

- [`head` arrives with the visit reply, so a buffer whose reply hasn't landed has none] → The command reports `File is not in a git repository` then; it's the same moment the mode line has no branch, and running it again after the reply works.
- [A repository whose `HEAD` can't be read gives no `Head`, so no relative path] → Rare; the error says so.
