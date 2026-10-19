## 1. Loaded projects and writing them

- [x] 1.1 `project.rs`: split `Project::check` out of `parse`; add `Loaded` and `sync(doc, project)`; add `toml_edit` to `aiditor-core`. Verified by a unit test that syncs a commented file (add, remove, reorder roots, set and clear role and agent) and keeps the comment, its result parsing back equal.
- [x] 1.2 `lib.rs`: `Msg::ProjectsLoaded { dir, projects }`, `FileOp::WriteProject`, `Msg::ProjectWritten`; core keeps `Vec<Loaded>` and the dir, `projects()` still lists the projects. Update the existing load test.
- [x] 1.3 `aiditor-services/projects.rs`: `read` returns `Loaded` with the text; `write` checks disk text, parses, checks dirs, creates the dir and writes (refusing an existing file when `old` is `None`); `files::run` dispatches it. Unit tests: create, refuse existing, refuse changed on disk, refuse missing dir, round trip.
- [x] 1.4 `aiditor` main/editor: pass the projects dir; existing `c_x_c_r_reloads_projects` test passes.

## 2. Commands and keys

- [x] 2.1 `ArgSpec::files: Option<usize>`; `find-file` uses `Some(0)`; find-file tests pass.
- [x] 2.2 `Ctx::edit_project`, `Ctx::write_new_project` and the seven commands in builtins with completion over project names, ad-hoc names and roots; bind them under `C-x p` in `keymap.toml`. Harness tests: each spec scenario (create, name taken, add/remove/set-role/default-agent on a file project with the harness faking the write, overlapping root refused, ad-hoc edits in memory, save, save of a filed project refused, open-file, unknown project, `C-x p` hint lists the keys).
- [x] 2.3 End-to-end in services: create then edit through `files::run` and read the dir back as the same project (AC 2).

## 3. Docs

- [x] 3.1 `docs/editor.md` Projects: the commands bullet records what was built and the defaults (prompt for project, `C-x p` keys, comments kept, disk check, reload after editing the file by hand).

## Workflow follow-up

- Archive the change after merge prep, per the implement skill.
