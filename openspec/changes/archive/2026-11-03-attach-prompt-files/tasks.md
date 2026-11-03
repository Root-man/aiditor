# Tasks

## 1. Attachment picker

- [x] 1.1 Register `agent-attach-file`, add its picker-local C-RET acceptance and insert existing chips in the prompt; verify files, images, directories, RET navigation and C-RET acceptance through core harness tests.
- [x] 1.2 Cover insertion amid text, output while picking, cancellation, transcript invocation and filesystem annotations through core tests; retain find-file regression coverage.

## 2. Integration and documentation

- [x] 2.1 Add GPUI test-platform coverage of the picker and chip display, and document the command and defaults in docs/editor.md; verify against SessionPrompt.dc.html.
- [x] 2.2 Run formatting, workspace clippy and tests, CI's architecture and Python checks, and OpenSpec validation; record manual frame/key-input verification required by the implement workflow.

Validation: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, core dependency/panic-profile checks, 21 Python benchmark-report tests, and strict OpenSpec validation passed. The sandboxed run timed out in the existing native-watcher integration test; that test and the full suite passed outside the sandbox. Three existing core tests remain ignored.

Manual verification: inspected the user's macOS screenshot against the attachment and chip design frames. It showed the session remaining visible, boxed file/directory chips, and the Attach picker with directory faces, count and aligned metadata. Deleted the screenshot after inspection as requested. The user confirmed RET directory navigation and C-RET attachment. Native desktop computer use is disabled in this session; Linux was not tested locally.

Separate OpenSpec verification: a fresh reviewer confirmed 4/4 tasks, 2/2 requirements and 7/7 scenarios, with no CRITICAL, WARNING or SUGGESTION findings. It independently passed the core suite (842 passed, 3 ignored), the focused GPUI attachment test, formatting, whitespace checks and strict OpenSpec validation. No suggestions were skipped.
