## Context

The core is pure: it reads nothing from disk and asks services for writes through effects. Launch-time files (settings, keymap, projects) are read by `main.rs` and handed to the core as messages. The services' `files::write` already writes atomically (temp file, sync, rename) behind a process-wide gate.

## Goals / Non-Goals

**Goals:** pins that survive a restart, written on each change, and dropped when their session is gone.

**Non-Goals:** sharing pins live between two running editors (last write wins), SQLite (deferred until state needs queries or grows; moving a small TOML file over is cheap), archive and delete (#481 and others).

## Decisions

- **File.** `sessions.toml` under `directories`' `data_local_dir` (the same as `data_dir` on macOS and Linux), an array of tables:
  ```toml
  [[pin]]
  agent = "claude"
  cwd = "/Users/me/app"
  session = "4f1c…"
  ```
  `cwd` is the list's primary root, so a project's answer only drops its own pins; the issue said per agent and session id, which alone would drop every other project's pins at each listing.
- **Reading.** `main.rs` reads the file before the core starts and sends `Msg::PinsLoaded(Result<Option<String>, String>)`; the core parses it with `toml` and logs a failure to `*Messages*`. The headless binary and tests send nothing and keep no path, so nothing is written.
- **Writing.** The core pushes `Effect::WritePins(String)` with the whole file; the runner knows the path (`Runner::pins`, `None` without a data dir) and writes on Tokio's blocking pool with a new `WriteExpectation::Overwrite`, creating the directory. Each write takes a number; a write that finds a newer number taken skips, under a lock held across the check and write, so the latest text wins whatever order the pool runs them in. A failure logs a warning, which reaches `*Messages*`.
- **Grouping.** `render_sessions` puts a pinned session in `Group::Pinned` but keeps the time its age group gives it; the today-colour applies to the Today group only, so a pinned row's time is dim, as frame 14 draws it.
- **Dropping.** `agent_listed`, for an `Ok` answer, removes the pins of that agent and the list's `cwd` not in the answer, writing and rebuilding other lists of the project when any went.

## Risks / Trade-offs

- Two editors running at once overwrite each other's pins → accepted for a personal editor; SQLite would fix it later.
- An agent that lists past its 50-page cap would drop pins on the cut pages → no agent comes close (65 sessions in one page measured).
- A corrupt file is overwritten by the next pin change → it holds only pins.
