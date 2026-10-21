# Design

## Context

The index (`Core::index`, #68) maps a project name to `(root path, sorted relative files)` per root, filled by `Msg::Indexed` for the latest walk only. `Ctx::try_call` opens a spec's prompt before the handler runs, and a spec's `completion` gets `&Core` only, so it can't start a walk. `read_string` makes a completing minibuffer only when there are candidates, unless `choices` (a handler's own question) is given; choices keep no history and never preview. `on_listed` shows how answers arriving later are pushed into the open matcher.

## Goals / Non-Goals

**Goals:** the finder over the index with the least new machinery; the walk starting on first use.

**Non-Goals:** history; feeding nucleo off-thread; a size limit or debounce for previewed files.

## Decisions

- **The handler asks its own question.** `project-find-file` has no spec prompt. Called with no strings, it starts `index_project` when the project has no index entry, then `read_string(command, args, "Find file in <project>: ", Some(&candidates))`; its answer calls it again with one string, which it visits. Alternative: a spec prompt plus a hook to run before prompting, or `completion` returning `Option` to allow an empty completing list: both touch every command's spec for one user. Cost: no `M-p` history, accepted. Its own question still previews: `Core::preview` skips handler questions only for commands that declare prompts, since a command with none has no other question to preview.
- **Root names from the index's own root paths**, via one function used both for candidates and for mapping an answer back, so the two always agree; it reuses `buffer::unique` for `<2>`. Before the first walk answers there are no index roots, so typed text falls back to the project's primary.
- **Answer to path:** split at the first `/`; if the head is a root's name, join the rest to that root, else join the whole text to the primary. A selected candidate always has a root head. Visit with `find_file` on the absolute path, which reuses a buffer visiting it in this workspace.
- **Late walk into an open finder.** `indexed`, after storing, pushes the stored files into the matcher when the minibuffer is open on `project-find-file` and the walk's project is the current workspace's (switching workspaces closes the minibuffer, so an open finder is always the current project's). A small `Core` method in `minibuffer.rs` injects candidates, keeping `Minibuffer`'s fields private.
- **Binding `C-x p f`**, Emacs's `project-find-file`.
- **The hook names a buffer or a file** (`Preview::Buffer` / `Preview::File`), so it stays `fn(&Core, &str)` and the core owns reading and windows. `C-x b` gives buffers; `find-file` and the finder give paths (`find-file` joins the candidate to the prompt's listing base; a directory gives none).
- **A file is read with `FileOp::Preview`, answered by `Msg::Previewed`**, the same read as `FileOp::Open` under another message, so a preview read can never be taken for a visit. Alternative: a flag on `Open`/`Opened`, touching every visit and its tests.
- **The preview buffer visits the file** (`create_for_file`, read-only), so `Buffers::visiting` finds a real buffer or the preview one alike, and `C-x b` lists never offer it while the minibuffer is open since none can open. It is killed before `RET` runs the command, so `find_file` reads the file again and makes a normal buffer. Alternative: adopt it on `RET` as consult does; saves one read but needs renaming and undoing read-only.
- **While a read is pending** the window keeps what it showed and `previewed` already holds the candidate, so nothing is asked twice; the answer is taken only when it is the awaited path (`reading`), and it resets `previewed` so the next `preview` shows it. A failed or binary read just restores the window.

## Risks / Trade-offs

- [100k paths pushed into nucleo on the core thread per open] → strings are pushed once per open; measured with a real large root during manual checks, and the injector is `Send` if it ever needs a thread.
- [The index goes stale until `C-x p i`] → noted in `docs/editor.md`; the watcher (#80) refreshes it.
- [Previewing reads whole files, however large] → `shortcut:` comment; add consult's size limit if big files make moving the selection slow.
- [A rebuild while the finder is open adds the new walk's files on top of the old ones] → duplicates until the prompt closes; rare, since `C-x p i` can't run from inside the minibuffer.
