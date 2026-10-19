# Tasks

## 1. Core format

- [x] 1.1 Add `aiditor-core/src/project.rs` with `Project`/`Root` (Serialize + Deserialize, `deny_unknown_fields`) and `Project::parse` via `settings::load`, checking non-empty name, absolute paths and one-line roles. Verify with unit tests: full file, minimal file, round trip through `toml::to_string`, and each invalid case naming its problem.
- [x] 1.2 Add `Msg::ProjectsLoaded`, `Core::projects` and the handler (replace, `report` each error). Verify with a core test: one ok and one error give one project and an echo line naming the file.

## 2. Loading

- [x] 2.1 Add `aiditor-services/src/projects.rs` `read(dir)`: sorted `*.toml`, parse, `is_dir` check, file-name prefix, missing dir is empty. Verify with tempdir tests: valid file, missing path, missing dir, non-toml file ignored.
- [x] 2.2 Send `Msg::ProjectsLoaded` at launch in `main.rs` and in the app's `Effect::LoadSettings` arm. Verify `cargo build` and the full check suite pass.

## 3. Docs

- [x] 3.1 Record the format, location and validation in `docs/editor.md`'s Projects section. Verify by reading the diff.
