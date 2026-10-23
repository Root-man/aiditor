# Tasks

These stages implement one change. Opportunity numbers refer to `proposal.md`; decisions refer to `design.md`. Every checkbox includes its completion check. Behavioral acceptance scenarios live in the nine delta specs; structural acceptance is the checklist in D11. All implementation tasks remain unchecked during proposal creation.

## 1. Establish controllable boundary tests (#6; D11)

- [x] 1.1 Add core-harness support for collecting effects and explicitly delivering delayed or reordered replies; verify a test can hold a file reply across workspace switching without executing filesystem code.
- [x] 1.2 Add a small services integration driver around the real core and production file/grep operations, allowing reply delivery to be held; verify one existing visit/save/index scenario through production services.
- [x] 1.3 Record which existing harness tests exercise filesystem contracts versus pure core transitions, alongside their migration destinations; verify every duplicated Save, WriteProject, Index, Check, Open, Preview, Revert, Diff, ListDir and grep behavior has an assigned real-service test or explicit-response replacement.

## 2. Stabilize project ownership and reconcile reloads (#13; D3)

- [x] 2.1 Introduce session-local project identity and migrate workspace bindings, index keys, project-write/index messages, command resolution and display callers; verify existing project/workspace tests and unchanged TOML round trips.
- [x] 2.2 Reconcile successful definitions by project-file path, preserve ad-hoc IDs and preserve an ID when an ad-hoc project is saved; verify same-file name changes and ad-hoc saves retain workspace/index ownership.
- [x] 2.3 Retain the last valid saved definition while bound, release removed definitions and indexes after their last workspace dies, and reattach reappearing paths; verify missing/invalid files, file renames and last-workspace cleanup scenarios.
- [x] 2.4 Preserve deterministic first-match name commands without rebinding existing owners, and bind multi-step operations to the selected project identity; verify duplicate names, reordered files and rename-during-operation scenarios.
- [x] 2.5 Guard project-write replies by entry revision and request identity, refreshing after a successful superseded write; verify delayed write completion cannot replace a newer reconciled definition.

## 3. Consolidate window configurations (#15; D10)

- [x] 3.1 Reuse `workspace::Windows` for the active core configuration and migrate direct window/tree/selection callers, display builders and fixtures; verify the existing core window, display and workspace tests pass with unchanged observable snapshots.
- [x] 3.2 Switch complete configurations and share killed-buffer replacement across active and inactive workspaces; verify split layouts, selected windows, points, marks, rings, scroll and marker release survive switching and workspace deletion.
- [x] 3.3 Preserve minibuffer origin restoration and cross-workspace buffer display through the common configuration; verify preview cancellation, stale click handling and killing a buffer shown in another workspace.

## 4. Bind file visits and grep destinations to intent (#1; D1)

- [x] 4.1 Add originating workspace/window and request identity to asynchronous visit effects/replies, and migrate every ordinary finder, project finder, project-file and grep caller plus runners; verify a valid completion opens in its issuing context with existing file validation.
- [x] 4.2 Invalidate pending visits on the navigation changes listed in the file-visiting delta, retaining ordinary-edit behavior; verify delayed replies across workspace/window switching, returning to the origin, buffer switching, origin deletion and a newer prompt.
- [x] 4.3 Attach deferred grep coordinates to the matching visit intent and remove the path-only jump slot; verify a newer visit to the same path retains its own point and a valid grep completion lands on the correct Unicode column.

## 5. Serialize writes and align project-write contracts (#2, #11; D2)

- [x] 5.1 Add explicit create-only versus checked-update expectations and typed write failures; verify existing-file conflict, create collision, unreadable-file and deleted-file recreation behavior through real services.
- [x] 5.2 Put buffer and project disk checks plus writes behind the shared services write gate, keeping encoding/validation outside and avoiding recursive locking; verify concurrent same-file, symlink-alias and hard-link saves preserve one complete winning edit and refuse the conflicting one.
- [x] 5.3 Use exclusive unique replacement temporary files with ownership-limited cleanup; verify a pre-existing temp survives, write failures clean only the operation's temp, and permission/link/in-place behavior remains covered.
- [x] 5.4 Replace project error-string rewriting with typed boundary formatting and migrate its divergent harness cases; verify deleted project recreation preserves comments, conflicts retain disk content and direct the user to `C-x C-r`, and create-only writes refuse a newly occupied path.
- [x] 5.5 Exercise queued saves across save-some, quitting and workspace killing; verify successful saves advance the action, failed saves retain the workspace/editor and edits, and edits made during a save remain modified.

## 6. Encapsulate indexes and coalesce walks (#7, #8; D4)

- [x] 6.1 Extract index state/transitions into a core module with private readiness, scope, generation, counts, missed paths and announcement state; migrate project/watcher callers and verify existing indexing and watch-limit tests.
- [x] 6.2 Keep one active walk and one latest pending refresh per project, forwarding explicit-user announcement intent; verify burst requests, changed queued scopes, released projects, stale replies and one final announcement using controlled delivery.
- [x] 6.3 Separate last accepted snapshot/counts from requested scope; verify same-scope re-walks preserve watches and mode lines, changed scopes wait for fresh counts, and partial-root errors retain current reporting behavior.
- [x] 6.4 Replace linear exact/descendant file scans with sorted lookups and preserve missed-path suppression; verify component-boundary cases such as `app`/`app2`, empty directories, ignored visited files and creates/deletes, and compare lookup work on a reproducible event/file fixture.
- [x] 6.5 Return kept directories with files from the shared walk and derive counts from that result; verify ignored trees, root inclusion, zero limits, shared roots and directory counts using real filesystem fixtures.
- [x] 6.6 Invalidate index filtering and clear missed suppression after relevant ignore-file changes, including excluded ignore files; verify newly included/excluded paths, manual reload of global excludes and the unchanged on-demand policy for unwatched projects.

## 7. Recover and accurately track native watches (#3, #4; D5)

- [x] 7.1 Carry accepted directory coverage and recovery intent from index results to watch reconciliation even for equal root/pattern sets; verify lost-directory-event recovery and same-scope continuity without dropping unrelated registrations.
- [x] 7.2 Detect replaced or invalidated directory registrations and re-establish them while preserving unaffected ones; verify delete/recreate within one debounce interval and later file notifications with deterministic policy tests and native watcher tests.
- [x] 7.3 Track desired, confirmed and uncertain backend changes with a narrow failure-injection seam; verify add, remove and commit failures remain pending and subsequent success repairs coverage.
- [x] 7.4 Add deadline-driven capped retries and first/changed-failure plus recovery reporting, including backend initialization failure; verify recovery without incoming events, duplicate-message suppression and cancelling retries when coverage is released.
- [x] 7.5 Derive visited-file fallback from confirmed root coverage and retain shared-root union/size-limit behavior; verify failed-root fallback, differing ignore patterns across shared roots and recovery that crosses `max_watched_files`.
- [x] 7.6 Exercise native coverage recovery on macOS and Linux using the existing CI matrix; verify later writes are observed after replacement/ignore changes and record platform-specific failure diagnostics if a check fails.

## 8. Coalesce and prepare previews off-thread (#5; D6)

- [x] 8.1 Add prompt-session/request identity with one active and one latest pending preview across closures/reopenings; verify rapid `a` to `b` to `c` selection reads the latest pending file and old same-path sessions cannot install replies.
- [x] 8.2 Move preview read validation, decoding and Rope preparation into services and return transferable prepared text without an unused content hash; verify UTF-8/BOM/CRLF, binary/missing/unreadable behavior and input handling while a worker is deliberately held.
- [x] 8.3 Install/release prepared previews through existing restoration and buffer ownership paths; verify at most one hidden preview buffer, same-file grep position changes, visited-buffer reuse, `C-g`, clicks, workspace switching and fresh ordinary visitation on `RET`.

## 9. Move runtime reload work out of the frontend (#9; D7)

- [x] 9.1 Add a shared background loader for a request-numbered settings/keymap/project bundle and adapt GPUI dispatch; verify a held reload permits input and the accepted components apply in their existing order with independent validation/fallback behavior.
- [x] 9.2 Coalesce repeat reload requests and reject obsolete bundles; verify two reloads cannot apply older or mixed-generation configuration after the latest request.
- [x] 9.3 Coordinate reload snapshots with project entry revisions and successful edits; verify both reload-before-write and write-before-reload reply orderings reconcile the current disk definition without replacing a newer accepted edit with an older snapshot.
- [x] 9.4 Update exhaustive runner/test effect handling while preserving the headless runner's current omitted watching/configuration capabilities; verify startup behavior, headless supported file/search effects and frontend reload tests.

## 10. Keep project finder mappings stable (#12; D8)

- [x] 10.1 Capture roots and root labels for each finder, using the shown index snapshot or initial pending scope as appropriate; verify typed `lib/new.rs` and duplicate-name `lib<2>/new.rs` resolve before the first walk completes.
- [x] 10.2 Use that mapping for preview, selected candidates and typed acceptance and reject incompatible late candidate injection; verify a project reload cannot reinterpret visible candidates and a newly opened finder uses its latest compatible snapshot.

## 11. Flush grep hits during file processing (#10; D8)

- [x] 11.1 Share the batch flush path between match processing, file completion and search completion; verify with controllable time that matching lines in one ongoing file scan produce an intermediate batch.
- [x] 11.2 Preserve ordered hits, search IDs, cancellation-before-send, exact limits and final completion; verify multi-root order, Unicode columns, cancellation after an intermediate batch and the 10,000-hit prompt cap with the production grep service.

## 12. Reuse Swiper query work (#14; D9)

- [x] 12.1 Capture a reproducible baseline for prompt-open and per-query costs on small and large multiline buffers, recording fixture size and build mode in the change's validation notes; verify the measurements execute real Swiper paths without a new dependency.
- [x] 12.2 Share a prompt-scoped compiled query across filtering, preview and acceptance, with only the necessary command/minibuffer seam changes; verify smart case, ordered words, exclusions, escaped separators, literal fallback and match point placement.
- [x] 12.3 Reduce avoidable line-text copying using the buffer snapshot and existing completion boundary; verify line order, trailing-newline handling, stable selection, history, mark and jump behavior through existing positive-behavior tests.
- [x] 12.4 Repeat the baseline fixtures and document the change in cost and remaining full-scan limit; verify results are comparable and no background filtering, global cache or size cutoff was introduced.

## 13. Finish the test-boundary cleanup (#6; D11)

- [x] 13.1 Complete the test migration inventory and remove duplicated filesystem/grep execution from the core harness; verify every migrated filesystem contract runs through production services and pure core tests use explicit replies.
- [x] 13.2 Extend `aiditor-services/tests/file_sync.rs` with controlled completion orderings and real shared-write behavior; verify cross-workspace saves, watcher recovery, failed operations and stale completions affect the intended buffers only.
- [x] 13.3 Check the structural acceptance checklist in the design against the finished tree; verify core dependency boundaries, private index transitions, stable project ownership, one window representation and shared Swiper query preparation.

## 14. Document and verify the whole change

- [x] 14.1 Update `docs/editor.md` with the chosen navigation cancellation, project reload/retention, watcher recovery and remaining performance limits; verify it agrees with all nine delta specs and distinguishes preserved M1 policy from behavior changed here.
- [x] 14.2 Run the repository's formatting, clippy, core dependency and panic-profile checks plus the full test suite on macOS and Linux; verify the existing CI checks pass and record exact results without treating planned checks as executed.
- [x] 14.3 Exercise the integrated GUI flows: rapid finder/grep selection, workspace switching during a read, concurrent same-file saves, configuration reload during typing, and directory replacement; verify buffers, prompt ownership and later disk notifications match the acceptance scenarios.
- [x] 14.4 Reconcile proposal IDs 1–15 against implementation and evidence, record remaining limitations in validation notes, and run strict OpenSpec validation; verify no opportunity, caller, fixture or acceptance scenario was silently omitted.
