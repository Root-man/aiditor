# Design

## Context

The core is a synchronous state machine: it returns `Effect`s, the app runs them on Tokio and services reply with `Msg`s. Per-project background work already follows this shape: `ensure_watch_indexes` starts an index for every project that has a workspace and none yet. The decisions are in docs/editor.md, Process environment (#267).

## Goals / Non-Goals

**Goals:** the capture, its fallback, per-project state, re-capture and `getenv`, all testable without a real profile.

**Non-Goals:** making spawns wait for a capture (no agent or formatter spawns exist yet), project-file and registry `env` overrides (#194 and the registry epic), direnv (#270), completion of variable names in `getenv`, re-capturing when a project file's primary root changes.

## Decisions

- **Core state.** `Core` gets `capture: bool` (off by default, so tests and benchmarks run no shells; `main` turns it on when stdout is not a terminal) and `environments: HashMap<ProjectId, Environment>`, where `Environment` holds the running capture's request number, if any, and the last answer's variables (`None` for the fallback), so a re-capture keeps the old variables until its answer arrives. Requests are numbered so a re-capture discards an earlier answer, as file index walks do.
- **Trigger.** `Core::handle` checks, after dispatch and next to `ensure_watch_indexes`, for projects that have a workspace and no entry, and pushes `Effect::CaptureEnvironment { project, request, dir }` for each. That covers the launch workspace and any later one without touching each place that opens a workspace.
- **Service.** `aiditor_services::environment::capture(shell, dir, timeout)` is async on `tokio::process`: `pre_exec(setsid)`, stdin null, stderr null, stdout piped. It reads stdout until the end marker or EOF under `tokio::time::timeout`, so a daemon a profile starts that keeps stdout open does not hold the capture. On timeout it sends SIGKILL to the process group (the shell leads its own session, so its pid is the group id). The child is not awaited after the end marker; Tokio reaps it, so logout scripts do not delay the result. `run(dir)` reads `SHELL` and uses the 5 s timeout; tests call `capture` with fake shell scripts and a short timeout.
- **Markers.** `printf` prints them, since `echo -n` differs between shells; the markers are distinctive ASCII strings that `env -0` output cannot contain in a `NAME=` position by accident. Entries that are not UTF-8 or have no `=` are skipped rather than mangled.
- **Messages.** The reply is logged to `*Messages*` (success with the variable count, failure with the reason and that the app's environment is used), not echoed, so a launch capture does not overwrite whatever the user is reading. `project-capture-environment` echoes `Capturing <project>'s environment...`.
- **`getenv`** reads `std::env::var` for `Inherited` or no entry; the core may read the process environment, it only must not depend on GPUI or Tokio.

## Risks / Trade-offs

- [A profile that takes over 5 s] → fallback with a note; the timeout is a constant to raise if it bites.
- [`SHELL` unset under some launchers] → `/bin/sh`, which on macOS reads no zsh profile; a `shortcut:` comment says to read the passwd entry if that shows up.
- [A redirected shell launch captures anyway] → accepted in #267: it costs time, not correctness.
