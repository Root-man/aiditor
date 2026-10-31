# Tasks

## 1. Pin lookup

- [x] 1.1 Add candidate enumeration and pure parsers for the supported language/tool mappings; unit tests cover all formats, multiple versions, system selection and invalid values.
- [x] 1.2 Read candidates through a file operation; service tests verify nearest pins, filename precedence, unrelated entries, malformed files and unpinned files.

## 2. Live mode lines

- [x] 2.1 Store version metadata and accept only current lookup replies; harness tests verify visible suffixes, fallback, inactive styling, narrow fitting and late replies.
- [x] 2.2 Add candidate paths to the watch set and refresh on matching reports/rescans; native watcher tests verify edits, atomic replacement, creation and deletion across workspaces with project watching disabled.
- [x] 2.3 Document the behavior and defaults in docs/editor.md; use a GPUI test to verify the version reaches painted mode-line text.

- [x] 2.4 Color every supported language family through named theme faces; verify name/version styling, inactive windows, theme overrides and bundled face completeness with tests.

## 3. Verification

- [x] 3.1 Pass cargo fmt, workspace Clippy/tests, the CI dependency/profile checks and benchmark reporting tests; validate the OpenSpec change.
