# Design

## Context

`Terminal` already holds `cwd: Option<PathBuf>`, set by OSC 7. `Core::directory()` (used by `find-file`, `project-add-root` and other relative-name prompts) and `vterm` each compute the current buffer's directory from its file only.

## Goals / Non-Goals

**Goals:** one place that answers "the current buffer's directory" for files and terminals.

**Non-Goals:** reading the shell's cwd from the OS (`/proc`, `proc_pidinfo`) when OSC 7 is absent; following `cd` without OSC 7.

## Decisions

- **Seed `cwd` with the spawn directory.** `vterm` sets the new terminal's `cwd` to the directory it spawns in, so "last OSC 7, else the start directory" is one field, not two. Alternative: a separate `start_dir` field, rejected as an extra field for the same answer.
- **Shared helper.** A `Core` method returns the current buffer's file directory or its terminal's directory, if any; `directory()` falls back to the process working directory and `vterm` to the project's primary root, keeping each caller's existing fallback.

## Risks / Trade-offs

- A spawn that fails leaves the seeded directory, but the buffer is killed on failure, so nothing reads it.
- A terminal created without `vterm` (tests, future agent terminals) has no directory until OSC 7 and falls back as a fileless buffer does.
