# Proposal

## Why

Projects can only be made or changed by hand-editing TOML files in the config dir (#56). Issue #58 asks for the project file to be created and edited entirely from the editor, and for an ad-hoc project (a folder opened without a project, #64) to be saved to `projects/`.

## What Changes

- New commands, each on `M-x` and under the `C-x p` prefix: `project-create`, `project-add-root`, `project-remove-root`, `project-set-role`, `project-set-default-agent`, `project-open-file`, `project-save`.
- Every command but `project-create` asks first for a project, completing over the loaded projects' names; there is no current project until workspaces (#64).
- Edits to a project with a file rewrite that file in place, keeping its comments and layout (`toml_edit`), and only when the file on disk is still the text that was loaded; the result is checked the way a loaded file is before it is written.
- The core keeps, for each project, the file it came from and its text, or nothing for an ad-hoc project. Ad-hoc projects are edited in memory and written by `project-save`; nothing makes one in the app until #64.
- A new project file is `projects/<name>.toml`, refused when that file exists, a project already has the name, or the name contains `/` or starts with `.`.

## Capabilities

### New Capabilities

### Modified Capabilities
- `projects`: adds creating, editing, opening and saving projects from the editor.

## Impact

- `aiditor-core`: `project.rs` (loaded projects, syncing a TOML document to a project), `lib.rs` (`Msg::ProjectsLoaded` carries the projects dir and file texts, new `FileOp::WriteProject` and `Msg::ProjectWritten`), `builtins.rs` (the commands), `command.rs`/`minibuffer.rs` (`ArgSpec::files` names the one prompt that completes file names), `keymap.toml`.
- `aiditor-services`: `projects.rs` reads file texts and writes project files; `files::run` carries out the write.
- `aiditor`: `read_projects` passes the projects dir.
- New direct dependency of `aiditor-core`: `toml_edit` (already in `Cargo.lock`).
- `docs/editor.md` Projects section.
