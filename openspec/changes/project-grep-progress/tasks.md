# Tasks

## 1. Prompt lifecycle

- [x] 1.1 Add display-only grep status using existing request and debounce state; verify running, growing count, done, empty, error, short input, cancel, history and supersession with core harness tests.
- [x] 1.2 Document the indicators in docs/editor.md and verify the obsolete searching-indicator limitation is removed.

## 2. Integration and validation

- [x] 2.1 Verify status, echo-line cursor placement and cancellation through GPUI's test platform.
- [x] 2.2 Run formatting, workspace clippy/tests and remaining CI checks; record manual-check limitations and obtain fresh OpenSpec verification.

Validation: formatting, workspace clippy, workspace tests with native filesystem access, 21 benchmark reporting tests, core dependency and panic-profile checks, and strict OpenSpec validation passed. The sandboxed workspace run could not observe the native watcher; the full native-access run passed. Three pre-existing manual/live-agent tests were ignored by the suite.

Manual: the user observed `[searching]` while `INVALID_SITE` waited, followed by results appearing together, and approved shipping with the delay investigation delegated separately. Native app control was unavailable to this agent; the remaining visual/key checks and Linux were not checked by hand. Core and GPUI tests cover empty results, errors, cancellation and cursor placement.
