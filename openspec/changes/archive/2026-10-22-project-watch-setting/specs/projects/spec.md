# Spec Delta

## MODIFIED Requirements

### Requirement: Project file format
A project SHALL be a TOML file in the `projects` directory of the editor's config dir with a `name`, an optional `default_agent`, an optional boolean `watch` defaulting to true, an optional `ignore` list of gitignore-syntax patterns, a `primary` table and zero or more `roots` tables. Each root SHALL have a `path` and a `role`; a missing role SHALL mean an empty one, and a missing `ignore` an empty list. A project written back out SHALL read back as the same project, and project commands SHALL leave `ignore` as it is in the file.

#### Scenario: Full file
- **WHEN** a project file sets a name, a default agent, a primary root and two additional roots
- **THEN** the loaded project has those values, the roots in file order

#### Scenario: Round trip
- **WHEN** a loaded project is written back to TOML and read again
- **THEN** the result equals the loaded project

#### Scenario: Minimal file
- **WHEN** a project file sets only a name and a primary path
- **THEN** the project has no default agent, no ignore patterns, no additional roots and an empty primary role

#### Scenario: Ignore patterns
- **WHEN** a project file sets `ignore = ["*", "!notes/"]`
- **THEN** the loaded project has those two patterns, in that order


### Requirement: Project commands are keys and M-x commands
The editor SHALL provide `project-create`, `project-add-root`, `project-remove-root`, `project-set-role`, `project-set-default-agent`, `project-set-watch`, `project-open-file`, `project-save`, `project-switch`, `project-index` and `project-find-file`, each runnable through `M-x` and bound under the `C-x p` prefix. Every command but `project-create`, `project-save`, `project-switch` and `project-find-file` SHALL first prompt `Project: ` completing over the loaded projects' names.

#### Scenario: Keys
- **WHEN** `C-x p` is pressed and the prefix hint shows
- **THEN** it lists a key for each of the eleven project commands

#### Scenario: Unknown project
- **WHEN** a project command is given a name no loaded project has
- **THEN** nothing changes and the echo area says there is no project with that name


## ADDED Requirements

### Requirement: Project watching is configurable and visible
The ad-hoc project rooted at HOME SHALL default to `watch = false`; other ad-hoc projects SHALL default to true. Saving an ad-hoc project SHALL retain its watch setting. `project-set-watch` (`C-x p w`) SHALL first prompt for a project, then `Watch: ` with true/false choices and its current value selected. Any other value SHALL be refused. It SHALL use the same checked, comment-preserving edit behavior as other project edits. Ordinary window mode lines SHALL show `[unwatched]` while the current workspace's project has `watch = false`.

#### Scenario: HOME launch
- **GIVEN** the editor is launched without a path
- **WHEN** its ad-hoc HOME workspace is shown
- **THEN** its project is unwatched and ordinary mode lines show `[unwatched]`

#### Scenario: Change and save
- **GIVEN** an ad-hoc project with watching enabled
- **WHEN** `C-x p w` sets it to false and `project-save` saves it
- **THEN** its project file loads with false and the mode lines show `[unwatched]`

#### Scenario: Saved project edits preserve comments
- **GIVEN** a loaded saved project has `watch = false # quiet`
- **WHEN** `project-set-watch` sets true
- **THEN** the file loads with true and retains `# quiet`, and its mode lines return to their watched state

#### Scenario: Invalid watch value
- **GIVEN** a project with watching enabled
- **WHEN** `project-set-watch` is given `yes`
- **THEN** it reports an error and the project retains true
