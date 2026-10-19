# projects Specification

## Purpose

The project file: a TOML file in the editor's config dir naming a primary repo, additional roots with role lines and a default agent, and how the editor loads and validates it.

## Requirements

### Requirement: Project file format
A project SHALL be a TOML file in the `projects` directory of the editor's config dir with a `name`, an optional `default_agent`, a `primary` table and zero or more `roots` tables. Each root SHALL have a `path` and a `role`; a missing role SHALL mean an empty one. A project written back out SHALL read back as the same project.

#### Scenario: Full file
- **WHEN** a project file sets a name, a default agent, a primary root and two additional roots
- **THEN** the loaded project has those values, the roots in file order

#### Scenario: Round trip
- **WHEN** a loaded project is written back to TOML and read again
- **THEN** the result equals the loaded project

#### Scenario: Minimal file
- **WHEN** a project file sets only a name and a primary path
- **THEN** the project has no default agent, no additional roots and an empty primary role

### Requirement: Invalid project files are rejected with a clear error
A project file that is not valid TOML, has a value of the wrong type, a key the editor does not know, a missing or empty name, a missing primary root, a path that is not absolute, a path that does not exist as a directory, a root that is the same as or inside another root of the same project, or a role spanning more than one line SHALL be rejected. The error SHALL name the file and the problem, be shown in the echo area and logged to `*Messages*`; errors from several files SHALL share the line. Valid files SHALL still load. Roots shared between different projects SHALL NOT be an error.

#### Scenario: Relative path
- **WHEN** a project file's primary path is `src`
- **THEN** the project is not loaded and the echo area names the file and says the path is not absolute

#### Scenario: Missing directory
- **WHEN** an additional root's path does not exist
- **THEN** the project is not loaded and the echo area names the file and the missing path

#### Scenario: Unknown key
- **WHEN** a project file sets `defualt_agent`
- **THEN** the project is not loaded and the error names the unknown key

#### Scenario: One bad file among good ones
- **WHEN** the projects directory holds one valid and one invalid file
- **THEN** the valid project is loaded and the echo area shows the invalid file's error

#### Scenario: Nested root
- **WHEN** a project's primary path is `/r/app` and an additional root is `/r/app/vendor/lib`
- **THEN** the project is not loaded and the error says `/r/app/vendor/lib is inside /r/app`

#### Scenario: Duplicate root
- **WHEN** two roots of a project have the same path
- **THEN** the project is not loaded and the error names that path

#### Scenario: Sibling with a shared prefix
- **WHEN** a project has roots `/r/app` and `/r/app2`
- **THEN** the project loads

#### Scenario: Root shared across projects
- **WHEN** two project files both list `/r/zed` as a root
- **THEN** both projects load

### Requirement: Projects load at launch and on C-x C-r
The editor SHALL read every `*.toml` file in the projects directory at launch and again when `C-x C-r` reloads settings, replacing the projects it had. A missing projects directory SHALL mean no projects and no error.

#### Scenario: No projects directory
- **WHEN** the config dir has no `projects` directory
- **THEN** there are no projects and no error is shown

#### Scenario: Reload picks up a new file
- **WHEN** a project file is added and `C-x C-r` is pressed
- **THEN** the new project is loaded
