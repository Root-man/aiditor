# Spec Delta

## MODIFIED Requirements

### Requirement: Project commands are keys and M-x commands
The editor SHALL provide `project-create`, `project-add-root`, `project-remove-root`, `project-set-role`, `project-set-default-agent`, `project-open-file`, `project-save`, `project-switch`, `project-index` and `project-find-file`, each runnable through `M-x` and bound under the `C-x p` prefix. Every command but `project-create`, `project-save`, `project-switch` and `project-find-file` SHALL first prompt `Project: ` completing over the loaded projects' names.

#### Scenario: Keys
- **WHEN** `C-x p` is pressed and the prefix hint shows
- **THEN** it lists a key for each of the ten project commands

#### Scenario: Unknown project
- **WHEN** a project command is given a name no loaded project has
- **THEN** nothing changes and the echo area says there is no project with that name
