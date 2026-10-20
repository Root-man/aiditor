# Design

## Context

The core keeps every buffer in one `Buffers` registry (`buffer.rs`) with globally unique names and a most-recent order. Buffers are made from many places (launch scratch, find-file, `C-x b` to a new name, recovery, the minibuffer, `kill_buffer`'s fallback scratch, `*Messages*`). One window tree is shared by everything; per-workspace trees are #63.

## Goals / Non-Goals

**Goals:** ownership on every buffer with no change at the creation sites; core functions to make and kill workspaces, tested through the harness.

**Non-Goals:** commands and keys (#64), per-workspace window configuration (#63), scoping `C-x b` and its completion (#66), the frontend strip (#65), asking about modified buffers before a workspace is killed (#64's command).

## Decisions

- **Ownership is stamped by `Buffers`.** `Buffers` holds the current `WorkspaceId`; `add` copies it into the new buffer's `workspace: Option<WorkspaceId>`. Every creation site then gets the right owner without passing it. `*Messages*` sets `None` right after it is made. Alternative: a workspace argument on `create`, touching every caller for no behavior difference.
- **Projects are held by name.** `Workspace { id, name, project: Option<String> }`. The project list is replaced wholesale on reload and edited in place, so an index or a clone would go stale; a name survives both and matches how project commands already pick a project ("the first of equal names winning"). `None` only for the launch workspace until #64 binds it.
- **Workspaces live in a `Vec` in creation order** with a current id, the order the strip (#65) shows. Killing the current workspace selects the one after it, or the one before when it was last, like a tab bar.
- **Kill reuses `Core::kill_buffer`.** Kill switches the current workspace first, then kills each owned buffer; `kill_buffer` already moves windows off a killed buffer, and its replacement is now filtered to the current workspace's buffers or `*Messages*`, falling back to a new `*scratch*` stamped with the current workspace.
- **Names stay globally unique.** A second workspace's scratch is `*scratch*<2>`, and a file open in two workspaces gives `a.txt` and `a.txt<2>`. Per-workspace names wait for #66, which scopes the switcher anyway.
- **`visiting(path)` filters by the current workspace.** That one filter gives the second buffer for a file open elsewhere.

## Risks / Trade-offs

- [The window tree is shared until #63] → a new workspace shows its scratch only in the selected window; other windows keep showing the old workspace's buffers until they are switched or #63 lands.
- [Kill does not ask about modified buffers] → no command calls it yet; #64's kill command must ask before calling it. Recovery files of killed buffers are deleted as for `kill-buffer`.
- [`C-x b` still lists every workspace's buffers] → #66.
