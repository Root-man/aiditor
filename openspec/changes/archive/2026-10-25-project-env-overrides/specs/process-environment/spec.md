## ADDED Requirements

### Requirement: Project file overrides
A project's environment SHALL be its captured environment, or the editor's own when nothing is captured or the capture fell back, with the variables of the project file's `env` table on top. A terminal started with `vterm` in a workspace of the project SHALL get those variables, overriding the editor's values; the terminal's own variables (`TERM` and the like) SHALL still win. A change to `env` SHALL apply to processes started after the project file is reloaded; running processes SHALL keep theirs.

#### Scenario: A variable reaches a terminal
- **WHEN** the project file sets `MIX_ENV = "test"` and `vterm` starts a terminal in a workspace of that project
- **THEN** the terminal is spawned with `MIX_ENV=test`

#### Scenario: An override wins over the captured value
- **WHEN** the project's captured environment has `PATH=/a` and the project file sets `PATH = "/b"`
- **THEN** `getenv` answered `PATH` echoes `/b`

#### Scenario: An override while a capture runs
- **WHEN** a capture of the project is still running and the project file sets `MIX_ENV = "test"`
- **THEN** `getenv` answered `MIX_ENV` echoes `test`

#### Scenario: Another project's variables stay out
- **WHEN** only project A's file sets `MIX_ENV` and `vterm` starts a terminal in a workspace of project B
- **THEN** the terminal is spawned without `MIX_ENV`
