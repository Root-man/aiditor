## MODIFIED Requirements

### Requirement: Project file format
A project SHALL be a TOML file in the `projects` directory of the editor's config dir with a `name`, an optional `default_agent`, an optional boolean `watch` defaulting to true, an optional `ignore` list of gitignore-syntax patterns, an optional `env` table of string variables, an optional `enabled_agents` list of agent names, a `primary` table and zero or more `roots` tables. Each root SHALL have only a `path`; a missing `ignore` an empty list, and a missing `env` no variables. A project written back out SHALL read back as the same project, and project commands SHALL leave `ignore`, `env` and `enabled_agents` as they are in the file.

#### Scenario: Full file
- **WHEN** a project file sets a name, a default agent, a primary root and two additional roots
- **THEN** the loaded project has those values, the roots in file order

#### Scenario: Round trip
- **WHEN** a loaded project is written back to TOML and read again
- **THEN** the result equals the loaded project

#### Scenario: Minimal file
- **WHEN** a project file sets only a name and a primary path
- **THEN** the project has no default agent, no ignore patterns, no env variables, and no additional roots

#### Scenario: Ignore patterns
- **WHEN** a project file sets `ignore = ["*", "!notes/"]`
- **THEN** the loaded project has those two patterns, in that order

#### Scenario: Env variables
- **WHEN** a project file has an `[env]` table setting `MIX_ENV = "test"`
- **THEN** the loaded project has the variable `MIX_ENV` with value `test`

#### Scenario: Project commands keep env
- **WHEN** a project file with an `[env]` table and a comment on one of its lines has a root added by a project command
- **THEN** the file still holds the `[env]` table and its comment

#### Scenario: Enabled agents
- **WHEN** a project file sets `enabled_agents = ["codex"]`
- **THEN** the loaded project has the list `codex`, and a project command that adds a root leaves the key in the file

### Requirement: Edit a project's roots and default agent
`project-add-root` SHALL add a directory, completing file names, as an additional root; a relative one is taken from the current buffer's directory and a trailing `/` dropped. `project-remove-root` SHALL remove one of the additional roots, never the primary. `project-set-default-agent` SHALL set the default agent, completing over the project's enabled agents, empty text removing it; a name that is not a registered agent, or a registered one the project does not enable, SHALL leave the project unchanged and say so in the echo area.

#### Scenario: Add a root
- **WHEN** `project-add-root` adds `/r/zed` to a project
- **THEN** the project lists `/r/zed` after its other roots

#### Scenario: Remove the primary
- **WHEN** `project-remove-root` is answered with the primary root's path
- **THEN** nothing changes and the echo area says it is not an additional root

#### Scenario: Clear the default agent
- **WHEN** `project-set-default-agent` is answered with empty text
- **THEN** the project has no default agent

#### Scenario: Unregistered default agent
- **WHEN** `project-set-default-agent` is answered with `gemini` and no agent `gemini` is registered
- **THEN** the project is unchanged and the echo area names `gemini` as not a registered agent

#### Scenario: Disabled default agent
- **WHEN** project `app` enables only `claude` and `project-set-default-agent` is answered with `codex`
- **THEN** the project is unchanged and the echo area says `"codex" is not enabled in app`

### Requirement: An unknown default agent is reported without rejecting the project
When project files load, at launch or on `C-x C-r`, a project whose `default_agent` names no agent in the settings in effect or one outside its enabled agents, or whose `enabled_agents` names an agent the settings do not register, SHALL be reported as `project <file>: <reason>` naming the agent, one report per agent, in the echo area and `*Messages*`, sharing the line with other errors. The project SHALL still load with everything else in the file, and SHALL keep the `default_agent` and `enabled_agents` values so writing the project back leaves them in the file; an unregistered name in `enabled_agents` enables nothing.

#### Scenario: Unknown default agent
- **WHEN** a project file sets `default_agent = "gemini"` and no agent `gemini` is registered
- **THEN** the echo area shows `project <file>: ` with a reason naming `gemini`, and the project is loaded with its roots

#### Scenario: Agent added later
- **GIVEN** a project file's `default_agent` named an unregistered agent `codex`
- **WHEN** the user adds `[agents.codex]` to settings and presses `C-x C-r`
- **THEN** no error about the project is shown

#### Scenario: Unknown enabled agent
- **WHEN** a project file sets `enabled_agents = ["claude", "gemini"]` and no agent `gemini` is registered
- **THEN** the echo area shows `project <file>: ` with a reason naming `gemini`, and the project loads with `claude` as its only enabled agent

#### Scenario: Default agent not enabled
- **WHEN** a project file sets `enabled_agents = ["claude"]` and `default_agent = "codex"`, with both registered
- **THEN** the echo area shows `project <file>: ` with a reason saying `codex` is not enabled, and `agent-start` preselects `claude`
