# Tasks

## 1. Review refinements

- [x] 1.1 Restrict owned inventory by type/session identity and derive counts from it; verify mixed-kind and zero-eligible-buffer acceptance.
- [x] 1.2 Retain session WHERE label faces and escaped text through snapshot and fitting; verify themes, control safety and narrow/wide restoration.
- [x] 1.3 Bind g and preserve project/workspace/buffer point identities in every sharing window on refresh with ancestor fallback; verify actual keys, reordering, removal and scroll behavior.
- [x] 1.4 Replace old spacing with real horizontal rules between every project; verify folding, refresh, resizing and navigation skip decorated separator rows.

## 2. Integration

- [x] 2.1 Update affected fixtures and docs/editor.md to the refined contract; verify focused core/frontend acceptance and strict delta validation.
- [x] 2.2 Run formatting, clippy, workspace tests, Python benchmark tests and CI dependency/profile guards; record evidence and platform limits.

## Validation evidence

- Required full `cargo test --workspace` passed outside the sandbox: 1,155 passed, 0 failed, 5 existing ignored. Log: `/tmp/aiditor-review-workspace-tests.log`.
- Focused workspace-list core acceptance: 29 passed. Log: `/tmp/aiditor-review-core-targeted.log`.
- GPUI workspace-list acceptance: 3 passed. Log: `/tmp/aiditor-review-frontend-targeted.log`.
- fmt, clippy with warnings denied, 21 Python benchmark tests, diff, dependency/profile guards and strict OpenSpec validation passed. Summary: `/tmp/aiditor-review-checks.md`.
- Native watcher tests require execution outside this environment's sandbox. The recorded full run used escalation; a sandbox retry is not equivalent validation.
- Native screenshots/manual input and Linux execution were skipped under the user's automated-only review scope. No flakes were waived, added or fixed.
- Strict validation's archive-order INFO is expected: these MODIFIED requirements refine the five still-active predecessor changes and must be archived after them.
