# Proposal

## Why

The grep service (#73) searches a project's roots off the core thread, but no command uses it, so there is still no way to grep across the repos of a project. Issue #74 adds the consult-ripgrep-style command: live results as you type, a preview of the selected hit and `RET` to jump to it.

## What Changes

- `project-grep` on `C-x p g` (project.el's key for its grep) prompts `Grep in <project>: ` for the current workspace's project and searches all its roots.
- The typed text is the one regexp the service takes. A search starts once the text has at least 3 chars and 200 ms passed since the last change (consult's `consult-async-min-input` and `consult-async-input-debounce`); shorter text cancels any search and empties the list.
- Hits are listed as `<root name>/<path>:<line>:<text>`, 1-based line, root names as `project-find-file` names them, in the service's order and not filtered again. The old list stays until the new search's first answer, so the list does not flash empty while typing. At most 10,000 hits; an invalid regexp shows its error in the echo area.
- The selected hit is previewed in the window the prompt was opened from: the file, with point at the start of the match.
- `RET` visits the file and puts point at the start of the match. No mark is pushed (there is no global mark ring yet).
- Leaving the prompt any way cancels a running search.
- The minibuffer's position preview generalises from "a position in the origin buffer" to "a position in a file", which swiper keeps using.
- The test harness carries out `Effect::Grep` with a small in-process search, so key-script tests cover search, move and jump.

Not in this change: consult's `#pattern#filter` split and multi-word input, highlighting the match, a "searching..." indicator, truncating long lines, editable results (#117), jump history.

## Capabilities

### New Capabilities

### Modified Capabilities
- `project-grep`: adds the `project-grep` command: prompt, debounced live search, the hit list, preview and jump.

## Impact

- `aiditor-core`: a `project-grep` command and `C-x p g` binding; the core keeps the running search's hits and takes `Msg::Grepped`; the minibuffer gets a file-position preview and a way to replace or extend a filtering prompt's candidates; a jump waits for the visited file's read.
- `aiditor-core` test harness: answers `Effect::Grep`.
- `docs/editor.md`: the command recorded under Search and navigation.
