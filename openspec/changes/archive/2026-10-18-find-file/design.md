# Design

## Context

Prompts come from `ArgSpec::prompts`; `ArgSpec::completion` gives a fixed candidate list, pushed into one `nucleo` matcher when the minibuffer opens, and `Core::poll_completion` reparses the pattern from the text after each message. `Core::open` visits a file given its bytes and hash; at startup `main` reads the file synchronously. Saves go out as `Effect::Save`, run on Tokio's blocking pool, and come back as `Msg::Saved`. #39 loads candidates on the UI thread; a directory listing must not.

## Goals / Non-Goals

**Goals:** `C-x C-f` with completion over the typed directory, listings and file reads off the UI thread, switching to an already visited file, a way to name a new file whose name fuzzy-matches another.

**Non-Goals:** `~` and `$VAR` expansion, `//` and `~/` resets in the text, `..` and symlink folding when comparing paths (so `/d/../d/a` and a link to a visited file make a second buffer), dired, directories listed first, hidden-file filtering, `find-file-other-window`, remembering the prompt's directory per buffer (Emacs's `default-directory`).

## Decisions

- **`ArgSpec::files`** marks a prompt as completing file names. `read_string` then starts the text as the current buffer's directory (`file.parent()`, else `std::env::current_dir()`), with `Path::join("")` adding the trailing `/`, and makes a `Completion` with `dir: Some(..)`. Alternative: a candidate function per directory; it would run on the UI thread, which is what #39 left to fix.
- **`Completion::dir`** is the directory part (text through its last `/`) whose listing the matcher holds or awaits. `poll_completion` splits the text; when the directory part changes it restarts the matcher, records the new part and pushes `Effect::ListDir`, so it now takes the effects list. A relative directory part is listed from the directory the prompt started at (`Completion::base`), the one `find-file` joins the answer onto. The pattern is the text after the last `/`. A `listed` flag drops a second answer for the same directory.
- **`Msg::Listed { dir, result }`** fills the matcher when the minibuffer still awaits `dir`; otherwise it is dropped, so a slow listing for a directory already left never shows. An error goes to the echo area.
- **`files::list`** in `aiditor-services` reads the directory, follows symlinks to tell directories (`fs::metadata`), appends `/` to them and sorts by name; non-UTF-8 names are shown lossily. The app runs it with `spawn_blocking`, like saves.
- **Full paths.** `selected_candidate` returns the directory part plus the entry for file prompts, so `TAB` and `RET` need no special case beyond `RET` on an entry ending in `/`, which calls `set_text` and keeps the prompt.
- **`minibuffer-exit-input`** (`M-RET`, in `minibuffer-completion`) is `exit-minibuffer` with the text instead of the selection, like Vertico's `vertico-exit-input`.
- **`minibuffer-directory-delete-char`** is bound to `DEL` in `minibuffer-completion` and falls back to `delete-backward-char`'s behavior unless the prompt completes files, point is at the end, there is no prefix or active mark, and the text ends in `/` or its last component plus `/` is among the matcher's injected items: the listing already loaded, so no new IO.
- **`find-file`** joins the answer onto the current buffer's directory (an absolute answer replaces it), switches to a buffer whose `file` equals that path, or pushes `Effect::Open(path)`. The app reads it with `files::read` on the blocking pool and sends `Msg::Opened { path, result }`. On arrival, a buffer that now visits the path (two finds raced) is switched to; a binary or non-UTF-8 file is refused with `<path>: <reason>`; otherwise `Core::open` visits it. `Core::open` keeps its command-line handling of refused files (they open read-only) and shares the switch to an already visited buffer, so opening one path twice now reuses its buffer.

## Risks / Trade-offs

- [An empty buffer-without-file directory comes from the process's working directory, which a GUI launched from Finder sets to `/`] → acceptable until projects give buffers a directory (M1).
- [A huge directory sends one big message] → fine for M0; stream entries if it bites.
- [`RET` with nothing listed yet returns the text, so typing fast and `RET` before the listing arrives visits the typed name] → the same as a name with no match, which is the expected answer.
