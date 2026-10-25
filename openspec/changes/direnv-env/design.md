# Design

## Context

`aiditor-services::environment::run` captures a project's environment; the app's `Effect::CaptureEnvironment` task runs it and sends `Msg::EnvironmentCaptured`, which the core keeps per project and notes in `*Messages*`. Zed's `load_direnv_environment` finds `direnv`, runs `direnv export json` with the shell's environment and `TERM=dumb` in the directory, bails on a non-zero exit and parses a `HashMap<String, Option<String>>`.

## Goals / Non-Goals

**Goals:** an allowed `.envrc` in the primary repo reaches the captured environment; a failure keeps the capture and says why in `*Messages*`.

**Non-Goals:** running direnv when launched from a shell (the shell's hook already ran, if at all) or after a fallback; watching `.envrc` for changes (`project-capture-environment` reloads); an `.envrc` in other roots.

## Decisions

- **A separate step after `run`**, `environment::direnv(&mut vars, dir)`, called by the capture task. The capture stays untouched and a direnv failure can't turn into a capture failure.
- **`direnv` is looked up in the captured `PATH`**, not the editor's: from the Dock, the editor's `PATH` lacks Homebrew's and Nix's directories. A plain loop over `PATH`'s directories for an executable file, rather than the `which` crate, which is not a dependency of `aiditor-services`.
- **Run with `env_clear()` then the captured variables**, so the process sees exactly the project's environment, plus `TERM=dumb` as Zed sets so direnv prints no colours.
- **A 30 s timeout with `kill_on_drop`**: a hung `.envrc` must not leave the project "still capturing" forever. Longer than the capture's 5 s because `use nix` or `use flake` can build on first load.
- **Output parsed as `BTreeMap<String, Option<String>>`** with `serde_json`, already a dependency. Empty output means no changes, as direnv prints nothing then.
- **`Msg::EnvironmentCaptured` gains `direnv: Option<String>`**, the failure; the core logs `Couldn't load direnv for <project>, using the shell environment without it: <reason>` after the capture line. The reason is direnv's trimmed stderr for a non-zero exit, which for a blocked `.envrc` says to run `direnv allow`.

## Risks / Trade-offs

- [A first `use flake` build longer than 30 s fails] → the note says so; `project-capture-environment` retries once it's cached.
- [direnv prints `.envrc`'s output on stderr even on success] → ignored on success, as Zed does.
