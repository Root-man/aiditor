## MODIFIED Requirements

### Requirement: Project file format
A project SHALL be a TOML file in the `projects` directory of the editor's config dir with a `name`, an optional `default_agent`, an optional boolean `watch` defaulting to true, an optional `ignore` list of gitignore-syntax patterns, an optional `env` table of string variables, a `primary` table and zero or more `roots` tables. Each root SHALL have a `path` and a `role`; a missing role SHALL mean an empty one, a missing `ignore` an empty list, and a missing `env` no variables. A project written back out SHALL read back as the same project, and project commands SHALL leave `ignore` and `env` as they are in the file.

#### Scenario: Full file
- **WHEN** a project file sets a name, a default agent, a primary root and two additional roots
- **THEN** the loaded project has those values, the roots in file order

#### Scenario: Round trip
- **WHEN** a loaded project is written back to TOML and read again
- **THEN** the result equals the loaded project

#### Scenario: Minimal file
- **WHEN** a project file sets only a name and a primary path
- **THEN** the project has no default agent, no ignore patterns, no env variables, no additional roots and an empty primary role

#### Scenario: Ignore patterns
- **WHEN** a project file sets `ignore = ["*", "!notes/"]`
- **THEN** the loaded project has those two patterns, in that order

#### Scenario: Env variables
- **WHEN** a project file has an `[env]` table setting `MIX_ENV = "test"`
- **THEN** the loaded project has the variable `MIX_ENV` with value `test`

#### Scenario: Project commands keep env
- **WHEN** a project file with an `[env]` table and a comment on one of its lines has a root added by a project command
- **THEN** the file still holds the `[env]` table and its comment

### Requirement: Invalid project files are rejected with a clear error
A project file that is not valid TOML, has a value of the wrong type, a key the editor does not know, a missing or empty name, a missing primary root, a path that is not absolute, a path that does not exist as a directory, a root that is the same as or inside another root of the same project, a role spanning more than one line, an ignore pattern that is not a valid gitignore pattern, or an `env` variable whose value is not a string, whose name is empty or holds `=` or NUL, or whose value holds NUL SHALL be rejected. The error SHALL name the file and the problem, be shown in the echo area and logged to `*Messages*`; errors from several files SHALL share the line. Valid files SHALL still load. Roots shared between different projects SHALL NOT be an error.

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

#### Scenario: Bad ignore pattern
- **WHEN** a project file sets `ignore = ["src/[z-a]"]`
- **THEN** the project is not loaded and the error names the file and the pattern

#### Scenario: Bad env variable
- **WHEN** a project file's `[env]` table sets `PORT = 4000` or `"A=B" = "c"`
- **THEN** the project is not loaded, the error names the file and the variable, and the other project files still load
