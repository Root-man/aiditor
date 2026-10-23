# Proposal

## Why

M1 added independent workspaces, project indexes, watchers, previews and search on top of the M0 core/services split. Several asynchronous operations still depend on whichever workspace is current when they finish, while duplicated state and test substitutes make lifecycle, concurrency and filesystem failures harder to handle consistently.

## What Changes

One change covers all 15 opportunities from the post-M1 exploration. The numbers below remain stable across the design and tasks; impact and complexity are relative estimates, not measured performance results.

| # | Opportunity | Impact | Complexity |
| --- | --- | --- | --- |
| 1 | Give asynchronous file visits and grep jumps an originating context and reject obsolete completions. | High | Medium |
| 2 | Serialize editor writes through conflict checking and replacement, and give temporary files unique names. | High | Medium |
| 3 | Reconcile watcher directory coverage after lost events, ignore changes and directory replacement. | High | Medium |
| 4 | Track confirmed watch registrations separately from requested coverage, retry failures and report them. | High | Medium |
| 5 | Coalesce preview requests and prepare preview text off the core thread. | High | Medium |
| 6 | Replace filesystem behavior duplicated in the core harness with production-service contract tests and controllable message delivery. | High | Medium |
| 7 | Give the file-index module ownership of its transitions and invariants. | Medium | Medium |
| 8 | Use sorted index lookups and coalesce repeated project walks. | Medium | Medium |
| 9 | Load configuration away from the frontend thread and apply reload results in request order. | Medium | Low |
| 10 | Stream grep hits during a file scan, preserving ordering, cancellation and the hit cap. | Medium | Low |
| 11 | Align project-write specifications, tests and errors with the accepted deleted-file recreation behavior. | Medium | Low |
| 12 | Resolve typed project-root prefixes before the initial index completes. | Medium | Low |
| 13 | Use stable project identity and reconcile reloads without orphaning or silently rebinding workspaces. | Medium | High |
| 14 | Reuse Swiper's compiled query and avoid unnecessary line copies; measure remaining scan cost. | Medium | Medium |
| 15 | Reuse one window-configuration representation for active and inactive workspaces. | Low | High |

Preserve existing keys, matching rules, file formats, undo/marker behavior, workspace-local buffers and same-scope watch continuity. Bound preview work by request count without adding a file-size cutoff. Include #15 in this change despite its lower priority. No new dependencies, persistent IDs, incremental index engine, live finder refresh or unrelated M2 work are proposed.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `file-visiting`: completion ownership and serialization of writes from independent buffers.
- `file-watching`: recovery of directory coverage and truthful registration failure handling.
- `file-index`: coalesced rebuilds and refresh after ignore-file changes.
- `minibuffer`: bounded preview work and prompt-specific completion identity.
- `settings`: responsive, ordered runtime reloads.
- `project-grep`: batches emitted during a file scan and context-bound accepted jumps.
- `projects`: reload reconciliation, stable identity and explicit deleted-file recreation.
- `file-finder`: root-prefix resolution before indexing and stable candidate-to-path mapping.
- `workspaces`: project bindings that survive reloads.

The harness, index-module extraction, Swiper cleanup and window representation are structural changes; their existing behavioral specs remain authoritative.

## Impact

Changes reach `aiditor-core` messages/effects, project and workspace state, visiting, minibuffer, watching, indexing, grep and Swiper; `aiditor-services` file/project writes, watchers, grep and integration tests; and the GPUI/headless effect runners. `docs/editor.md` will record the settled lifecycle and reload policies during implementation. macOS and Linux remain supported. This proposal changes no project code or main specifications; implementation must preserve the existing cross-platform checks and add targeted race/failure coverage.
