# Spec Delta

## ADDED Requirements

### Requirement: Workspace project bindings survive reload
A live workspace SHALL remain bound to the same project across configuration reloads, independent of project display-name changes or duplicate names. A valid definition reloaded from the same project-file path SHALL update the bound project. If that file is missing or invalid, its last valid definition SHALL remain available while the workspace is live. Updating a project's name SHALL NOT rename the workspace or its scratch buffer. Reconciliation SHALL preserve the workspace's buffers, selected window, split layout, point, marks, scroll and jump history.

#### Scenario: A project is renamed in its definition
- **GIVEN** workspace `task` has a split layout and is bound to project `old`
- **WHEN** the same project file reloads with name `new`
- **THEN** `task` keeps its name, scratch name, buffers and layout, and its project label becomes `new`

#### Scenario: A missing definition still supports project commands
- **GIVEN** a workspace's saved project file has been deleted
- **WHEN** configuration is reloaded and the workspace runs a project finder or grep
- **THEN** the command uses the last valid project's roots and settings

#### Scenario: A duplicate name appears
- **GIVEN** a workspace is bound to a saved project and another project with the same name loads earlier in name-resolution order
- **WHEN** configuration is reloaded
- **THEN** the workspace still uses its original project's roots and retains its window configuration
