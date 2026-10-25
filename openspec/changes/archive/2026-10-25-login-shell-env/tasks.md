# Tasks

## 1. Service

- [x] 1.1 Add `aiditor-services/src/environment.rs` with `capture(shell, dir, timeout)`, the env parser and `run(dir)`; verify with tests using fake `SHELL` scripts that print noise and exit non-zero, hang (killed within the timeout, process group gone), exit without markers, and with a fish-named script receiving `emit fish_prompt`

## 2. Core

- [x] 2.1 Add `Effect::CaptureEnvironment`, `Msg::EnvironmentCaptured`, the `capture` switch and per-project `environments` state with captures started for projects that have a workspace; verify with harness tests: launch captures once, a second workspace on the same project does not, capture off starts nothing
- [x] 2.2 Handle replies: store a success, fall back on an error with a `*Messages*` line, ignore stale requests; verify with harness tests
- [x] 2.3 Add `project-capture-environment` and `getenv`; verify with harness tests for re-capture picking up new output, the launched-from-a-shell echo, a captured value, an inherited value, `not defined` and a running capture

## 3. App

- [x] 3.1 Turn capture on in `main.rs` when stdout is not a terminal, and run the effect on Tokio in `editor.rs`, sending the reply on the services channel; verify by building and by a manual launch from the Dock (`getenv PATH` shows the login shell's PATH)

## 4. Docs

- [x] 4.1 Note what was built under Process environment in `docs/editor.md`; verify the section reads correctly
