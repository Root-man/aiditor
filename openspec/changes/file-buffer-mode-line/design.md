# Design

## Context

`display::build` makes the mode line; it sees the buffer, point and mark, but no projects, and `frame()` adds `[unwatched]` after it. The file service runs `FileOp`s on the blocking pool. The watcher takes `WatchSet.files`, watches the parent of any that no root covers, and reports changes to those files, dropping other files in that directory. `.git` is never in a walk. `Buffer.file` is set only when the buffer is created.

## Goals / Non-Goals

**Goals:** everything the issue names is decided in the core and readable by display tests; a checkout outside the editor is noticed by the existing watcher.

**Non-Goals:** a VCS abstraction or anything beyond git; repository state other than the branch and whether the file differs; the language version; noticing a repository created after the file was visited.

## Decisions

**A `file` arm in `mode_line`.** `mode_line` branches on `buffer.file`; every other buffer goes through the code it has today. Alternative: one layout for all buffers, which would change terminals and sessions that have their own tasks.

**`build` takes `roots: &[PathBuf]`.** The left group needs the path relative to its root, and patching it into the first segment in `frame()` as `[unwatched]` is added would miss `Core::display` and split one segment's logic. `Core::roots()` gives the current workspace's project's roots. A file under none shows its absolute directory.

**Position from what `build` already laid out.** `Top` when `first == 0`; bottom when the loop ended at the buffer's end (`pos == len_chars`), which also holds when the final newline's empty line is below the window, as in Emacs. `wrap_lines` stops truncating, so `build` can tell that rows were cut. `n%` is chars above the window over all chars, as Emacs's `%p`.

**`.git/HEAD` read directly.** It is one small file, and a branch needs nothing more. `files::head(path)` walks up to the first `.git`; a `.git` file (worktree, submodule) is followed through its `gitdir:` line, relative to it. `ref: refs/heads/x` gives `x`, another ref its full name, and a detached hash its first 7 chars.

**`git status` for ` ±`, added in review.** Whether a file differs from its commit means reading git's objects, packs included, so the alternatives are `gix`/`git2`, a large dependency, or a process. `git --no-optional-locks status --porcelain -- <name>` runs in the file's directory on the file service. `--no-optional-locks` stops it rewriting `.git/index`, which is now watched and would loop; `--literal-pathspecs` keeps `*` in a name from matching others; `core.fsmonitor=false` stops a repository's config running a command on visit. Untracked counts, as the whole file differs from the commit. Spawn failure counts as unchanged, so a missing git costs only the marker.

**`.git/index` watched too.** A commit moves the branch's ref, not `HEAD`, and always rewrites the index; so does `git add`. A report of the visited file re-asks as well, which covers saves.

**`head_asked` swept in `handle`.** `Buffer.head: Option<(PathBuf, String)>` holds the `HEAD` path and branch. After each message the core sends one `FileOp::Head` for the file buffers not yet asked about, as `ensure_watch_indexes` does for projects, so every way of creating a file buffer is covered. A report of a buffer's `HEAD`, or a rescan, clears `head_asked`, which re-asks.

**`HEAD` and `index` join `WatchSet.files`.** That makes `.git` a non-recursive fallback directory and reports `HEAD`, with no new watcher concept. A fallback directory reports every file in it, so `keep` drops paths under a `.git` that aren't watched files; git rewrites them on every command. `on_files_changed` takes these paths out before the index's staleness check, or each would re-walk its project once.

**`mode-line-buffer-id` face.** Bold, as Emacs's face of that name, because frames 1 and 9 draw the name bold and no existing face is. Frame 12 has none, so `docs/editor.md`'s face table gains it.

**Both marks when both are true.** A recovered buffer can be read-only and modified; the issue lists them independently, so `● ⊘`.

**`UTF-8 CRLF`, not `CRLF`.** Frame 9 draws the pair as one segment, and #155 adds encodings to it. A BOM stays out: it is still UTF-8.

## Risks / Trade-offs

- [A repository created after the visit isn't seen] → until a rescan or the next visit; `shortcut:` comment. Watching every ancestor for `.git` costs more than this is worth.
- [`.git` watched on macOS inside a recursive root] → a second, non-recursive registration; `pending` is a set, so one event is reported once.
- [One process per file on each index change] → a commit with many buffers open runs them in sequence on one blocking task; `shortcut:` comment.
- [A repository's `filter.*.clean` config can still run during status] → only for a config already on this machine, as any shell prompt showing git status does; `core.fsmonitor`, which runs on every status, is off.
- [Reftable repositories keep a placeholder in `HEAD`, `ref: refs/heads/.invalid`] → the mode line shows `.invalid`; `shortcut:` comment, until one is seen in use.
- [A wrapped final line longer than the laid-out columns can read as `Bot` early] → only with a line longer than the window's whole text area, already cut there.
