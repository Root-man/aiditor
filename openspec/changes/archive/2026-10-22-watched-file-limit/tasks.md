# Tasks

## 1. Configuration and counts

- [x] 1.1 Add settings default and optional project override; verify parsing, validation, serialization and preservation tests.
- [x] 1.2 Extend asynchronous index replies with kept-directory counts; verify ignored files/directories and empty directories with service tests.

## 2. Watch policy

- [x] 2.1 Preflight active watched projects and share effective eligibility across root watches, index refresh and mode line; verify startup, threshold equality, overage, override, reload, visited-file fallback, dynamic crossing and deduplicated diagnostics.
- [x] 2.2 Record defaults and reevaluation behavior in docs/editor.md; verify against implementation and specs.

## 3. Verification

- [x] 3.1 Run fmt, workspace clippy/tests, CI structural checks and strict OpenSpec validation.
