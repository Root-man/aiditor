# Design

## Context

See `proposal.md` for motivation and the two delta specs for behavior. Inspection at `a7abcea` found:

- `builtins.rs` registers all three commands. `reload-settings` calls `Ctx::reload_configuration`; `project_commands.rs` also requests reloads after project-write conflicts.
- `configuration.rs::configuration_loaded` rejects stale bundles before dispatching settings, keymap and project results. Those handlers in `lib.rs` report diagnostics through `Core::report`; a successful read can still contain TOML, binding or project validation errors. `Core::echo` already logs to `*Messages*`.
- `workspace.rs::list_workspaces` only formats an echo string. Workspaces hold stable `WorkspaceId` and `ProjectId` values, and buffers carry workspace ownership.
- `visiting.rs` already provides normal buffer switching, buffer-kill replacement, and a read-only, fileless, refillable `*Diff*` pattern. `frame.rs::display_buffer` deliberately leaves focus elsewhere, so it does not match an immediately keyboard-operated list.
- `ArgSpec::preselect` and minibuffer completion support initial candidate selection. Project prompts currently return names, and `Ctx::try_call` resolves the first matching name into `Args.project` before asking the second question. Preselection alone would therefore choose the wrong identity for equal project names.

## Goals / Non-Goals

**Goals:** Keep behavior in the portable core; reuse ordinary buffers, modes, keys, completion and workspace switching. Cover asynchronous reload acknowledgement and project identity in focused tests.

**Non-Goals:** No new UI framework, service protocol, dependency or persistence. No general purpose table browser, list mutation commands, folding or live subscriptions. Do not investigate the user's exact Doom key binding: the requested hierarchy and workspace-only activation define the behavior.

## Decisions

### 1. Announce application, not the reload request

Add confirmation at the end of the accepted-bundle path in `configuration_loaded`, after its existing stale-request and project-revision guards. Isolate that bundle's message accumulation from any earlier echo message before dispatching its three components, then use the existing `report` path for diagnostics and `echo("Configuration reloaded")` only when the bundle produced no diagnostics. Earlier messages already remain in `*Messages*`. Keep component parsing and fallback in their existing handlers and leave direct startup messages unchanged.

This avoids announcing success merely because file reads succeeded, or using an unrelated old echo message as evidence of failure. Emitting in the command handler would be premature; a new service-level success result would miss core validation. Confirmation does not wait for indexing or watch results, which retain their own later messages.

### 2. Render an ordinary, selectable workspace list buffer

Replace the echo formatter with a workspace-local buffer in `workspace-list-mode`, named `*Workspaces*` with normal unique-name suffixes. Find it by mode and owner, as the diff view does, refill it with `Buffer::set_text`, and select it through normal buffer switching in the current window. It has no file, is read-only and has no edit history. Reopening refreshes this buffer; `q`/`C-g` kills it using existing replacement behavior. This is a temporary ordinary buffer, not a second global buffer or a layout overlay.

Proposed rendering (spacing and tree glyphs are presentation details):

```text
Workspaces — RET switch, n/p move, q quit

aiditor
├── * testing
│   ├── *scratch*<testing>
│   └── workspace.rs
└── review
    └── *scratch*<review>
notes
└── writing
    ├── *scratch*<writing>
    └── ideas.md
```

Group by `ProjectId`, never by display name. Project order follows first occurrence in the workspace creation sequence; preserve workspace creation order inside each group. Add primary paths to duplicate project names. Include listed owned buffers in name order, excluding leading-space internal buffers, all workspace-list buffers and global messages. Escape embedded line breaks/tabs in displayed names so one label cannot fabricate another selectable row; keep identity separate from displayed text.

Keep a small per-list mapping of workspace line positions to `WorkspaceId`. Only these rows are navigation and activation targets; never parse names back out of rendered text. Use ordinary point as the visual selection and ensure navigation brings it into view. This needs no GPUI changes or generic tree model.

### 3. Keep list actions local and identity based

Register local commands for next/previous workspace row, visit and quit, bound in `[workspace-list-mode]` in the bundled keymap. Next/previous skips informational lines and clamps at the ends. `RET` only resolves a mapped workspace row; an informational row gets `Select a workspace row`. Ordinary text motion or clicks can still position point for reading and copying, but they do not make project or buffer rows activation targets.

Validate that the target `WorkspaceId` still exists before changing the view. On a valid visit, dismiss the invoking list with normal buffer-kill behavior, then call the existing workspace switch by ID; accepting the current workspace only dismisses the list. Existing workspace layout, point and mark restoration remain authoritative. Direct workspace switches while the list is open continue to behave like switches from any ordinary buffer; the list remains an owned snapshot until refreshed or dismissed. Clear row metadata when its buffer or owner is killed, including through ordinary `kill-buffer` and workspace teardown.

Selecting the list in the existing window avoids creating a split and an extra window-restoration mechanism. Dismissal uses the normal most-recent eligible buffer, not a promise to restore a separately saved window state.

### 4. Preselect the active project and carry its identity through acceptance

Use `ArgSpec::preselect` for `workspace-new`'s project prompt only, selecting the candidate matching `current_workspace().project`; leave the name prompt and other commands' defaults alone. Keep the input initially empty so choosing another project still uses ordinary fuzzy completion and navigation.

For that first prompt, retain each project candidate's `ProjectId` alongside its completion entry, and copy the accepted candidate's ID into `Args.project` before calling the command again. The existing `try_call` guard then preserves it through the name prompt. Candidate metadata must travel with the matched entry, not the visible match index, because filtering reorders results. Direct textual calls and literal-input acceptance keep existing name lookup and error behavior. The selected project is checked again at creation; it must not silently fall back to another equal-named project if removed. Keep this identity support scoped to workspace creation rather than redesigning all project commands.

Preselecting a name without retaining identity is insufficient for duplicate names. Skipping the project prompt would prevent the explicitly requested choice of another project.

## Risks / Trade-offs

- A list can become stale while another window changes workspaces or buffers → refresh on each invocation and validate workspace IDs on activation. Automatic refresh is outside this change.
- A generated list must not become dirty or capture ordinary editing keys globally → read-only buffer plus a mode-local map; test insertion rejection and normal keys outside the mode.
- Killing a list through another route can leave row metadata → tie cleanup to the existing buffer teardown path and test kill/reopen and owner teardown.
- Runtime reload can complete while a prompt is open → use the existing echo-area mechanism; do not interrupt or replace the prompt. The message is logged through the normal path.
- Snapshot grouping changes the old global creation-order presentation → intentionally update `List the workspaces`; creation order remains within each project.
- The list occupies the selected window, and dismissal follows normal buffer replacement rather than exact window rollback → document this small, standard-buffer behavior and preserve user buffer contents and split structure.

## Scope and structural acceptance checklist

- [x] Update `configuration.rs`, its tests, and only the diagnostic integration needed in `lib.rs`.
- [x] Update workspace command registration and workspace-list behavior in `builtins.rs` and `workspace.rs` (a small dedicated list module is acceptable if it keeps the existing file readable).
- [x] Add list row state and teardown integration in core/buffer lifecycle code; reuse `visiting.rs` switching and kill behavior.
- [x] Update `minibuffer.rs` completion data and acceptance narrowly for workspace-new project identity, retaining existing `Args.project` behavior.
- [x] Add the local keymap and focused core tests; update the old echo-output test rather than retaining conflicting expectations.
- [x] Update the configuration and workspace command descriptions in `docs/editor.md` during implementation.
- [x] Leave service effects, GPUI rendering, config formats, exports and dependencies unchanged unless a concrete implementation need is established.

## Migration Plan

No migration is required. Existing command names and global keys remain. Implement and test the three behaviors, update documentation, and run a manual UI smoke check on macOS and Linux. Reverting the change restores the prior commands without touching user data. The full M1 gate in #85 remains separate.
