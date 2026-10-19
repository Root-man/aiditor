## MODIFIED Requirements

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
