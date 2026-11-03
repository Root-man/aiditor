# Tasks

## 1. Reference picker

- [x] 1.1 Extend shared project finder setup to offer indexed files and directories for references, retain captured root mapping and fill on first-use replies; verify with harness tests for multiple roots, duplicate names, empty directories, delayed indexing and changed roots.
- [x] 1.2 Register and bind `agent-reference` to `@`, replace its literal with the selected chip and keep it on cancellation; verify insertion amid text, streamed output, resource-link/image sending, unmatched input and read-only transcript protection through harness tests.
- [x] 1.3 Record built behavior in docs/editor.md and verify it matches the issue and frame 6b.

## 2. Validation

- [x] 2.1 Run formatting, workspace clippy/tests, CI structural checks and Python benchmark tests; record results.
- [x] 2.2 Check rendered picker and chips against frame 6b and exercise actual @/RET/C-g input; capture and judge the frame, or obtain user screenshots and key-check results if native computer use is unavailable.

## Check results

- macOS: all eight focused reference harness tests pass.
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` pass.
- CI dependency/profile checks, all 21 Python benchmark-reporting tests, `git diff --check` and strict OpenSpec validation pass.
- User-provided native screenshots inspected: the picker shows both root labels, directory styling, `dir` and file-age annotations; the prompt shows the file and directory chips. Both screenshot files were deleted after inspection at the user's request. Native computer control is unavailable here; the user confirmed that real-key `C-g` closes the picker and leaves literal `@x` after typing `x`.
- Separate OpenSpec review: all five tasks, both requirements and all nine scenarios verified; no critical findings, warnings or suggestions. Its workspace test rerun passed outside the sandbox; restricted runs timed out waiting for native watcher events. No flake entry was added for that environment-specific failure.
