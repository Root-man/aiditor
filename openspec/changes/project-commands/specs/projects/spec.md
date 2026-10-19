## ADDED Requirements

### Requirement: Project commands are keys and M-x commands
The editor SHALL provide `project-create`, `project-add-root`, `project-remove-root`, `project-set-role`, `project-set-default-agent`, `project-open-file` and `project-save`, each runnable through `M-x` and bound under the `C-x p` prefix. Every command but `project-create` SHALL first prompt `Project: ` completing over the loaded projects' names.

#### Scenario: Keys
- **WHEN** `C-x p` is pressed and the prefix hint shows
- **THEN** it lists a key for each of the seven project commands

#### Scenario: Unknown project
- **WHEN** a project command is given a name no loaded project has
- **THEN** nothing changes and the echo area says there is no project with that name

### Requirement: Create a project
`project-create` SHALL prompt for a name and a primary root directory, completing file names, and write a project file `<name>.toml` in the projects directory with that name and primary and an empty role. It SHALL refuse a name that is empty, contains `/` or starts with `.`, a name a loaded project already has, and a file that already exists, writing nothing.

#### Scenario: New project
- **WHEN** `project-create` is answered `app` and `/r/app/`
- **THEN** `projects/app.toml` is written, it loads back as a project named `app` with primary `/r/app`, and `app` is among the loaded projects

#### Scenario: Name taken
- **WHEN** `project-create` is answered with the name of a loaded project
- **THEN** no file is written and the echo area says the project exists

#### Scenario: No projects directory yet
- **WHEN** the config dir has no `projects` directory and a project is created
- **THEN** the directory is made and the file written in it

### Requirement: Edit a project's roots, roles and default agent
`project-add-root` SHALL add a directory, completing file names, with a role as an additional root; a relative one is taken from the current buffer's directory and a trailing `/` dropped. `project-remove-root` SHALL remove one of the additional roots, never the primary. `project-set-role` SHALL set any root's role. `project-set-default-agent` SHALL set the default agent, empty text removing it.

#### Scenario: Add a root
- **WHEN** `project-add-root` adds `/r/zed` with role `GPUI source` to a project
- **THEN** the project lists `/r/zed` after its other roots with that role

#### Scenario: Remove the primary
- **WHEN** `project-remove-root` is answered with the primary root's path
- **THEN** nothing changes and the echo area says it is not an additional root

#### Scenario: Clear the default agent
- **WHEN** `project-set-default-agent` is answered with empty text
- **THEN** the project has no default agent

### Requirement: Edits are checked and keep the rest of the file
An edit to a project that has a file SHALL rewrite that file, keeping its comments, key order and every part the edit does not change. An edit SHALL be refused, with the file unchanged and the reason in the echo area, when the result would be rejected as a project file (an overlapping root, a missing directory, a role on more than one line) or when the file on disk differs from the text last loaded or written. A refused edit SHALL leave the loaded project unchanged.

#### Scenario: Comments survive
- **WHEN** a project file has a comment line and a root is added
- **THEN** the written file still has the comment

#### Scenario: Overlapping root
- **WHEN** `project-add-root` adds a directory inside the primary root
- **THEN** the file is unchanged and the echo area says the path is inside the primary

#### Scenario: Changed on disk
- **WHEN** the project file was changed on disk after it was loaded and a role is then set
- **THEN** the file keeps the disk version and the echo area says it changed on disk

### Requirement: Open a project file
`project-open-file` SHALL visit the project's file in a buffer, as `find-file` does. For a project without a file it SHALL fail, saying the project has no file.

#### Scenario: Visit
- **WHEN** `project-open-file` is answered with a project read from `projects/p.toml`
- **THEN** the current buffer visits `projects/p.toml`

### Requirement: Ad-hoc projects are edited in memory and saved
A project without a file (ad-hoc) SHALL be edited in memory by the edit commands, with the same checks except the directory and disk checks. `project-save` SHALL complete over the ad-hoc projects and write the chosen one to `<name>.toml` in the projects directory, with the same refusals as `project-create`; afterwards it SHALL be a project with that file. A saved file SHALL load back as the same project.

#### Scenario: Save
- **WHEN** an ad-hoc project with a role and an additional root is saved
- **THEN** `projects/<name>.toml` is written and reading it gives a project equal to the ad-hoc one

#### Scenario: Save a project with a file
- **WHEN** `project-save` is answered with a project that already has a file
- **THEN** nothing is written and the echo area says the project is already saved
