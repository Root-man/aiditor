# Design

## Context

- `aiditor_services::files::head(file)` finds the nearest `.git` at or above a file's directory (following a `.git` file's `gitdir:`), and reads the branch from `HEAD` as the mode line shows it. `changed(file)` runs `git status --porcelain` for one file with `--no-optional-locks` and `core.fsmonitor=false`, after refusing repositories whose own config defines a `filter.*`.
- File service operations are `FileOp` variants run by `files::run`, each answered by one `Msg`.

## Goals / Non-Goals

**Goals:** the operation and its service tests, per `specs/git-status/spec.md`.

**Non-Goals:** any caller or UI (the diff view, M9), refreshing on watcher reports, submodule contents, conflict details beyond the two state letters, ignored files.

## Decisions

- **A `FileOp`, not a new service.** `FileOp::GitStatus(Vec<PathBuf>)` → `Msg::GitStatus(Vec<(PathBuf, Result<Option<GitStatus>, String>)>)`, shaped like `FileOp::Head`/`Msg::Heads`. `Ok(None)` is "not a repository"; `Err` is git's failure. No request id until a caller needs to drop stale answers.
- **Branch from `HEAD`, not from `--branch`.** Porcelain v2's `# branch.head` says `(detached)`; the mode line shows the short hash. `head()` is split into `repository(dir)` (root, `HEAD` path, branch) so a root directory and a file share it.
- **Paths as git gives them.** `GitStatus.repository` is the directory holding `.git`; each file's path is relative to it, as porcelain v2 reports, kept as raw bytes (`OsStr::from_bytes`) so a non-UTF-8 name is the name on disk. `-z` keeps spaces, quotes and non-ASCII unquoted.
- **Limited to the root.** Run in the root with pathspec `.` and `--literal-pathspecs`, so a root below its repository's top shows only its own files, and two roots in one repository don't repeat each other's.
- **States as an enum.** `GitChange { Modified, TypeChanged, Added, Deleted, Renamed, Copied, Unmerged, Untracked }`, `None` for git's `.`. Untracked is `unstaged: Some(Untracked)`, `staged: None`, so the list's groups read straight off the two fields.
- **Concurrency.** One scoped thread per root (`std::thread::scope`); answers keep the roots' order.
- **Safety as `changed()`.** Same flags, and the repository-scope filter check is shared: a repository that defines its own filter answers an error instead of running it.

## Risks / Trade-offs

- [A repository with a local-scope filter (e.g. `git lfs install --local`) gets no status] → Same rule as the mode line's ` ±`; the error names why. Lift it if it shows up.
- [Submodules' own configs could still name an fsmonitor or filter] → `-c` reaches subprocesses through `GIT_CONFIG_PARAMETERS`, which covers fsmonitor; a submodule's filter is a shortcut noted in code.
