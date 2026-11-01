# Tasks

## 1. Core coverage and documentation

- [x] 1.1 Test named-theme syntax attribute inheritance and explicit false overrides; verify core settings tests pass.
- [x] 1.2 Extend display coverage to italic comments and bold keywords, with reloads disabling both and invalid bare colours retaining settings; verify core display tests pass.
- [x] 1.3 Align docs/editor.md with syntax faces and verify bundled and personal theme syntax entries are already face tables.

## 2. Frontend and integration verification

- [x] 2.1 Test font runs in an open source buffer before and after style reloads through GPUI's test platform; verify the frontend test passes.
- [x] 2.2 Run cargo fmt, workspace clippy and tests, CI structural checks and Python benchmark tests; record results.
- [x] 2.3 Check native italic comments, bold keywords and reload behavior, or obtain the user's manual results if native computer use is unavailable.

## Verification results

- cargo fmt --all --check, workspace clippy with -D warnings, CI dependency/profile checks, 21 Python benchmark tests, and strict OpenSpec validation passed.
- Workspace tests passed outside the sandbox with the listed integrated_finders_visits_saves_and_directory_recovery flake excluded after its native-watcher-readiness failure. No new flaky-test entry.
- Sandboxed native service watcher tests timed out; the isolated failing watcher test and the full service suite passed outside the sandbox.
- User confirmed all four native manual checks passed: italic comments and bold keywords, disabling styles while keeping colours, rejecting bare colours while retaining accepted settings, and restoring styles on reload. Scratch fixture: /tmp/aiditor-335-review. The user approved shipping.
- Fresh OpenSpec verification reported no critical issues, warnings or suggestions. Its focused syntax, settings, display and GPUI tests, formatting, diff check and strict validation passed.
