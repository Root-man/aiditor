# Spec Delta

## ADDED Requirements

### Requirement: An unknown default agent is reported without rejecting the project
When project files load, at launch or on `C-x C-r`, a project whose `default_agent` names no agent in the settings in effect SHALL be reported as `project <file>: <reason>` naming the agent, in the echo area and `*Messages*`, sharing the line with other errors. The project SHALL still load with everything else in the file, and SHALL keep the `default_agent` value so writing the project back leaves it in the file.

#### Scenario: Unknown default agent
- **WHEN** a project file sets `default_agent = "gemini"` and no agent `gemini` is registered
- **THEN** the echo area shows `project <file>: ` with a reason naming `gemini`, and the project is loaded with its roots

#### Scenario: Agent added later
- **GIVEN** a project file's `default_agent` named an unregistered agent `codex`
- **WHEN** the user adds `[agents.codex]` to settings and presses `C-x C-r`
- **THEN** no error about the project is shown

## MODIFIED Requirements

### Requirement: Edit a project's roots, roles and default agent
`project-add-root` SHALL add a directory, completing file names, with a role as an additional root; a relative one is taken from the current buffer's directory and a trailing `/` dropped. `project-remove-root` SHALL remove one of the additional roots, never the primary. `project-set-role` SHALL set any root's role. `project-set-default-agent` SHALL set the default agent, completing over the registered agents' names, empty text removing it; a name that is not a registered agent SHALL leave the project unchanged and say so in the echo area.

#### Scenario: Add a root
- **WHEN** `project-add-root` adds `/r/zed` with role `GPUI source` to a project
- **THEN** the project lists `/r/zed` after its other roots with that role

#### Scenario: Remove the primary
- **WHEN** `project-remove-root` is answered with the primary root's path
- **THEN** nothing changes and the echo area says it is not an additional root

#### Scenario: Clear the default agent
- **WHEN** `project-set-default-agent` is answered with empty text
- **THEN** the project has no default agent

#### Scenario: Unregistered default agent
- **WHEN** `project-set-default-agent` is answered with `gemini` and no agent `gemini` is registered
- **THEN** the project is unchanged and the echo area names `gemini` as not a registered agent
