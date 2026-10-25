# Proposal

## Why

Launched from the macOS Dock or a Linux desktop launcher, aiditor gets launchd's or the session's minimal environment, so the agents and formatters coming in M3 would not find `claude`, `node`, `mix` or `cargo`. The capture details were decided in #267 (docs/editor.md, Process environment); this change builds the capture itself (#268).

## What Changes

- At launch the app checks whether stdout is a terminal. If it is, nothing is captured and the inherited environment is used.
- Otherwise, when a project first has a workspace, its environment is captured in the background: the user's shell runs as an interactive login shell in the project's primary repo, in a new session with stdin from `/dev/null`, printing `env -0` between two markers (fish runs `emit fish_prompt` first). Only what lies between the markers is parsed, even when the shell exits non-zero, and `SHLVL` is dropped.
- A shell that hangs past 5 s has its process group killed; a hang, a failure or output without both markers falls back to the app's own environment, with a note in `*Messages*`. A success is noted there too.
- The result is kept per project for the session.
- New command `project-capture-environment` re-captures the current workspace's project, for processes started afterwards.
- New command `getenv` (as in Emacs) prompts for a variable name and echoes its value in the current project's environment: captured, or the app's own when inherited or fallen back.

## Capabilities

### New Capabilities
- `process-environment`: capturing the login shell environment per project, the launch check, the fallback, re-capturing, and reading a variable with `getenv`.

### Modified Capabilities

## Impact

- `aiditor-core`: a new message and effect, per-project environment state, the two commands.
- `aiditor-services`: a new `environment` module that runs and parses the capture.
- `aiditor`: the launch check in `main.rs`, and running the effect in `editor.rs`.
- `docs/editor.md`: Process environment notes what was built.
- Nothing consumes the environment yet: terminals keep starting login shells with the app's environment; agents and formatters will read it when they land.
