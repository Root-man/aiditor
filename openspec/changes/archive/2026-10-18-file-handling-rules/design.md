## Context

Today `main.rs` reads files with `read_to_string` (refusing non-UTF-8 by exiting) and saves with `std::fs::write`, which truncates in place. The display builder emits whole lines and the frontend clips them (`display.rs`, a `shortcut:` for #112).

## Goals / Non-Goals

**Goals:** settle the file rules #46 implements and the long-line behavior for M0 and M5.

**Non-Goals:** implementing any of it; encodings other than UTF-8; very long line performance (minified files).

## Decisions

- **UTF-8 only in M0.** Detecting encodings needs `encoding_rs` and per-buffer write-back; deferred to #155. Invalid UTF-8 is treated as binary, so no lossy decode can be saved over a file.
- **Line endings as Emacs's `buffer-file-coding-system`.** Keeping `\r` in the text would show in motion and editing; always writing LF would rewrite whole CRLF files in diffs.
- **Final newline added**, as Emacs's text and prog modes do.
- **Atomic saves with an in-place fallback.** Emacs writes in place by default, which keeps links and mode for free but leaves a truncated file if the editor dies or the disk fills mid-write. Rename keeps the old file intact until the new one is complete; copying the mode, resolving symlinks first and writing in place for hard-linked files keep what in-place writes kept. Owner and group are not copied (a rename by a non-owner can't keep them anyway).
- **Auto-hscroll on point's line in M0.** It fixes the off-screen cursor with a per-window offset instead of soft wrap's visual lines, which stay in M5. `current-line` keeps the rest of the window readable.
- **Emacs-like wrap in M5**: wrapping by default, word wrap for prose, `toggle-truncate-lines` per buffer.

## Risks / Trade-offs

- Rename changes the inode → the watcher sees a replace; already covered by comparing disk hashes.
- Refusing non-UTF-8 files blocks editing Latin-1 files until M5 → rare in the target repos.
