# Tasks

## 1. Core completion display

- [x] 1.1 Expose visible candidates' match byte ranges from the displayed nucleo snapshot; verify core tests cover command initials, empty/changed and multi-atom queries, Unicode and scrolled rows.
- [x] 1.2 Verify the shared behavior reaches buffer, project-file and directory-file prompts through harness tests, preserving annotations and basename matching.

## 2. Frontend and documentation

- [x] 2.1 Render match ranges with completion-match while preserving selection backgrounds and dimmed annotations; verify GPUI tests exercise input and rendered text runs.
- [x] 2.2 Update docs/editor.md for #201 and the confirmed fuzzy-only scope; verify OpenSpec strict validation passes.

## 3. Integration verification

- [x] 3.1 Run cargo fmt, workspace clippy and tests plus the CI dependency/profile and Python checks; resolve failures under the implement skill's flake policy.
- [x] 3.2 Complete acceptance checks available through the harness and GPUI, document remaining manual steps, and obtain fresh-subagent OpenSpec verification before local commit and user review.

## Acceptance checks

The user confirmed the native checks passed on 2026-10-31, after running `cargo run -p aiditor -- --config-dir /tmp/aiditor-201-check Cargo.toml`: `M-x pff` highlighted the three initials with a dim key annotation; `C-x b ct` highlighted the buffer name; `find-file` and `project-find-file` highlighted `Cargo.toml`; moving the selection retained the match face and clearing the query cleared the highlights. Each prompt was cancelled with `C-g`. Native app control was unavailable to the implementing agent; no manual checks remain outstanding.

Fresh-subagent verification found all four scenarios covered, with no implementation defects, warnings or suggestions. Its clean host-permission workspace run passed 900 tests with three existing intentional ignores; formatting, clippy, strict OpenSpec validation, CI structural checks and 21 Python tests also passed. Native watcher tests timed out in sandboxed runs and passed with host permissions.
