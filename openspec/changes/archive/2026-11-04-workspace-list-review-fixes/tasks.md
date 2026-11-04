# Tasks

## 1. Review refinements

- [x] 1.1 Restrict owned inventory by type/session identity and derive counts from it; verify mixed-kind and zero-eligible-buffer acceptance.
- [x] 1.2 Retain session WHERE label faces and escaped text through snapshot and fitting; verify themes, control safety and narrow/wide restoration.
- [x] 1.3 Bind g and preserve project/workspace/buffer point identities in every sharing window on refresh with ancestor fallback; verify actual keys, reordering, removal and scroll behavior.
- [x] 1.4 Replace old spacing with real horizontal rules between every project; verify folding, refresh, resizing and navigation skip decorated separator rows.

- [x] 1.5 Reproduce repeated fitted rows before editing, prevent list wrapping before layout/scroll/motion, and verify exact logical/shaped row counts across wrap toggles, font sizes, uneven splits, resize and scrolling.

## 2. Integration

- [x] 2.1 Update affected fixtures and docs/editor.md to the refined contract; verify focused core/frontend acceptance and strict delta validation.
- [x] 2.2 Run formatting, clippy, workspace tests, Python benchmark tests and CI dependency/profile guards; record evidence and platform limits.

## Earlier review-fix validation

- Required full `cargo test --workspace` passed outside the sandbox: 1,155 passed, 0 failed, 5 existing ignored. Log: `/tmp/aiditor-review-workspace-tests.log`.
- Focused workspace-list core acceptance: 29 passed. Log: `/tmp/aiditor-review-core-targeted.log`.
- GPUI workspace-list acceptance: 3 passed. Log: `/tmp/aiditor-review-frontend-targeted.log`.
- fmt, clippy with warnings denied, 21 Python benchmark tests, diff, dependency/profile guards and strict OpenSpec validation passed. Summary: `/tmp/aiditor-review-checks.md`.
- Native watcher tests require execution outside this environment's sandbox. The recorded full run used escalation; a sandbox retry is not equivalent validation.
- Native screenshots/manual input and Linux execution were skipped under the user's automated-only review scope. No flakes were waived, added or fixed.
- Strict validation's archive-order INFO is expected: these MODIFIED requirements refine the five still-active predecessor changes and must be archived after them.

## Single-row regression evidence

- Before the product edit, `/tmp/aiditor-wrap-reproduction.log` records expected logical rows 0–8 but repeated fragments at a 900px uneven split.
- The same core acceptance passes after the shared wrap decision excludes fitted list snapshots. Current check records will be `/tmp/aiditor-wrap-checks.md` and `/tmp/aiditor-wrap-workspace-tests.log`.

- Current required unsandboxed workspace run: 1,156 passed, 0 failed, 5 existing ignored across 16 groups; `/tmp/aiditor-wrap-workspace-tests.log`.
- Current workspace-list core acceptance: 30 passed; GPUI workspace-list acceptance: 3 passed. The latter asserts exact logical row sequences and shaped row counts at 12px/28px fonts and after resize/folding.
- Current fmt/clippy/Python21/strict delta/dependency/profile guards pass; `/tmp/aiditor-wrap-checks.md` records commands and logs. Native screenshots/manual input and Linux execution remain excluded by the authorized automated scope.
