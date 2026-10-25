# Proposal

## Why

direnv's hook runs at a prompt, which the capture's `-c` never draws, so a project whose primary repo has an `.envrc` misses its variables when the editor is not launched from a shell. docs/editor.md (Process environment) settles that direnv is loaded separately with `direnv export json` in the primary repo, as Zed does (#270).

## What Changes

- After a successful capture, when `direnv` is on the captured `PATH`, the editor runs `direnv export json` in the project's primary repo with the captured environment and applies its result: set variables are set, `null` ones removed.
- A failure (an `.envrc` that is not allowed, a non-zero exit, output that is not a JSON object of strings and nulls, a timeout) keeps the captured environment and adds a line to `*Messages*` naming the project and the reason.
- No `direnv` on the captured `PATH`, a failed capture, or a launch from a shell runs nothing.

## Capabilities

### New Capabilities

### Modified Capabilities
- `process-environment`: a capture loads direnv into the project's environment.

## Impact

- `aiditor-services`: `environment` gains the direnv step.
- `aiditor-core`: `Msg::EnvironmentCaptured` carries a direnv failure for `*Messages*`.
- `aiditor`: the capture task runs the direnv step after the capture.
- `docs/editor.md`: Process environment notes what was built.
