# Design

## Context

The list (`crates/aiditor-core/src/diff.rs`) keeps each line's `Row` (root, group, file with its repository path). Git runs only in the file service (`crates/aiditor-services/src/files.rs`), with `--literal-pathspecs` and a refusal when the repository's own config defines a filter. Yes/no questions re-invoke their command with the answer in `Args::strings` (`read_string` with `["No", "Yes"]`).

## Goals / Non-Goals

**Goals:** file and heading actions from the list, one git run per action.

**Non-Goals:** the change or region at point in the diff window (#511, which takes `C-c s`/`C-c k` there); turn-diff reverts (#513); deleting directories an untracked delete leaves empty.

## Decisions

- **One service op with ordered steps.** `FileOp::GitUpdate { list, repository, steps: Vec<(GitStep, Vec<PathBuf>)> }`, `GitStep` = `Stage` (`git add -A --`), `Unstage` (`git reset -q --`), `Restore` (`git checkout -q --`), `Delete` (remove the file), answered by `Msg::GitUpdated { list, result }`. A root discard is `[Restore, Delete, Unstage]`: restoring first keeps a staged change in the working tree. `git reset` over `git restore --staged` because it works on an unborn branch. Alternative, an op per action: a root discard would need two round trips and ordering in the core.
- **The core picks the paths from the row at point and the status.** A root has one repository, so an action never spans two. Paths are the rows' `file` (relative to the repository) plus a staged rename's `old_path`, both read from the status the list last showed.
- **Unsaved edits** are checked in the core against any buffer visiting the file, before asking or running anything; unstaging touches only the index and isn't checked.
- **Refresh re-asks the shown diff** by dropping it when a status answer re-renders the list; `sync_diff` then shows point's row again. This also fixes `g` leaving a stale diff.
- **Point after a refresh** goes to the nearest still-listed file after its old row in the old order, else before, so staging down `Changes` walks it.

## Risks / Trade-offs

- A later step failing after an earlier one succeeded (restore ok, unstage failed) leaves git changed while the list isn't refreshed → the error is echoed and `g` shows the truth.
- Discarding acts on the status last listed; a file changed again since then is restored all the same → the question names what is lost, as listed.
