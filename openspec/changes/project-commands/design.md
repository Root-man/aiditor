# Design

## Context

The core holds `projects: Vec<Project>` from `Msg::ProjectsLoaded`, which the app builds with `aiditor_services::projects::read`. A `Project` does not know its file, and the core does not know the projects directory. File IO runs as `Effect::File(FileOp)` through `files::run`, which the app and the headless binary already share. `ArgSpec::files` makes every prompt of a command complete file names, but the project commands mix a project-name prompt with a directory prompt.

## Goals / Non-Goals

**Goals:** one write path for every command; no file clobbered by a stale in-memory copy; hand-written comments kept.

**Non-Goals:** reloading projects when the visited project file is saved (the user presses `C-x C-r`); a current project; creating ad-hoc projects (#64); checking `default_agent` against an agent registry; prefilling the role prompt with the current role.

## Decisions

- **Loaded projects carry their file.** `project::Loaded { project, file: Option<(PathBuf, String)> }`, the path and the text last read or written; `None` is ad-hoc. `Msg::ProjectsLoaded { dir: Option<PathBuf>, projects: Vec<Result<Loaded, String>> }`, `dir` being the projects directory, `None` without a config dir (create and save then fail with `No config directory`). The text is what the staleness check and the in-place edit need, so no hash is kept.
- **Commands change a `Project`; one function writes it.** Each edit command clones the project, changes the clone, checks it with `Project::check` (the checks `parse` runs, split out so ad-hoc projects get them without a file), then `Ctx::edit_project` keeps it. For an ad-hoc project that replaces it in memory. For a file it parses the loaded text into a `toml_edit::DocumentMut`, `project::sync`s the document to the new project and emits `FileOp::WriteProject { path, old: Some(text), text: new }`. Alternative rejected: per-command document edits, which would also need a separate in-memory path for ad-hoc projects.
- **`sync(doc, project)`** sets `name`, `default_agent` (removed when `None`), the primary's `path` and `role` (removed when empty) only where they differ, and rebuilds `[[roots]]` in the project's order from the old tables matched by path, so a kept root keeps its comments. As a self-check, `Project::parse` of the result must equal the project.
- **New files** (`project-create`, `project-save`, through `Ctx::write_new_project`) are `toml::to_string(&project)` written with `old: None`; the service refuses an existing file, makes the directory, and writes through `files::write` with no expected hash, which refuses a file that appeared meanwhile.
- **The service checks before writing.** `projects::write(path, old, text)` parses `text` and checks the roots are directories as `read` does, then writes through `files::write` (atomic, with the hash of `old` as the expected one, its "changed on disk" error ending `reload with C-x C-r`), and replies `Msg::ProjectWritten(Result<Loaded, String>)`, the path inside `Loaded::file`. `files::run` dispatches `FileOp::WriteProject` to it. On `Ok`, the core replaces the entry with that file, or else the ad-hoc entry with that name, or else appends, and echoes `Wrote <path>`; on `Err`, it echoes `project <file>: <reason>`, as loading does.
- **`ArgSpec::files: Option<usize>`**, the index of the prompt that completes file names; `find-file` becomes `Some(0)`. A completion function returning no candidates gives a plain prompt, so the free-text prompts (role, agent) of a completing command show no empty list. Smallest change that lets one command mix prompts.
- **Roots typed at a prompt** are joined to the current buffer's directory and normalized with `components().collect()`, which drops a trailing `/` and `.`.
- **Keys:** `C-x p c` create, `C-x p a` add root, `C-x p r` remove root, `C-x p l` set role (role line), `C-x p d` default agent, `C-x p o` open file, `C-x p s` save. `C-x p` is project.el's prefix.
- **Names:** the first project with a name wins. `shortcut: duplicate names across files are not caught (#56), so a command acts on the first; check names at load if that bites.`

## Risks / Trade-offs

- [Writing while the visited project file has unsaved edits] → the disk check catches a saved edit; an unsaved buffer can later be saved over the command's write, which `FileOp::Save`'s own hash check then refuses.
- [`toml_edit` reformats a value it sets] → only values that changed are set.
