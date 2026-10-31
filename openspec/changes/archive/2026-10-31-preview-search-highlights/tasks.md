# Tasks

## 1. Match overlays

- [x] 1.1 Expose each command's compiled matching semantics and retain grep's displayed matcher until replacement results arrive; verify smart-case, query replacement and Swiper semantic tests.
- [x] 1.2 Thread the actual preview window's matcher into display construction, overlay match faces on visible characters and update callers/benchmarks; verify face, Unicode, scroll/wrap, split and prompt-cleanup tests.

## 2. Integration and documentation

- [x] 2.1 Cover the preview-to-render path with GPUI test-platform assertions and document #339 in docs/editor.md; verify frontend tests and documentation against the acceptance scenarios.
- [x] 2.2 Run cargo fmt, workspace clippy/tests, CI structural checks and benchmark-reporting tests; verify all checks pass and record any required manual check.

## Validation results

- Formatting, workspace clippy, workspace tests, CI structural checks, 21 benchmark-reporting tests and strict OpenSpec validation passed on macOS.
- The sandboxed full run timed out waiting for a native watcher; the individual test and full workspace suite passed outside the sandbox. No flake was added.
- GPUI tests cover simulated Swiper input, shaped match runs, acceptance/abort cleanup and a real grep-service preview.
- Step 5 live GUI check: the user reported all checks passed using the fixture at `/tmp/aiditor-339-manual/project/`: match contrast, selection movement, acceptance/abort cleanup, grep across files, smart case and split isolation. Native-app control is disabled for the agent.
