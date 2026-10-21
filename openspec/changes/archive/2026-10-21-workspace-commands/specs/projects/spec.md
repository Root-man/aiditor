# Spec Delta

## MODIFIED Requirements

### Requirement: Projects load at launch and on C-x C-r
The editor SHALL read every `*.toml` file in the projects directory at launch and again when `C-x C-r` reloads settings, replacing the saved projects it had. Ad-hoc projects SHALL stay. A missing projects directory SHALL mean no saved projects and no error.

#### Scenario: No projects directory
- **WHEN** the config dir has no `projects` directory
- **THEN** there are no saved projects and no error is shown

#### Scenario: Reload picks up a new file
- **WHEN** a project file is added and `C-x C-r` is pressed
- **THEN** the new project is loaded

#### Scenario: Reload keeps ad-hoc projects
- **WHEN** the editor was started with a folder no saved project contains, giving an ad-hoc project, and `C-x C-r` is pressed
- **THEN** the ad-hoc project is still there and its workspace is still bound to it

### Requirement: Project commands are keys and M-x commands
The editor SHALL provide `project-create`, `project-add-root`, `project-remove-root`, `project-set-role`, `project-set-default-agent`, `project-open-file`, `project-save` and `project-switch`, each runnable through `M-x` and bound under the `C-x p` prefix. Every command but `project-create`, `project-save` and `project-switch` SHALL first prompt `Project: ` completing over the loaded projects' names.

#### Scenario: Keys
- **WHEN** `C-x p` is pressed and the prefix hint shows
- **THEN** it lists a key for each of the eight project commands

#### Scenario: Unknown project
- **WHEN** a project command is given a name no loaded project has
- **THEN** nothing changes and the echo area says there is no project with that name

## ADDED Requirements

### Requirement: Switch project
`project-switch` (`C-x p p`) SHALL prompt `Switch to project: `, completing over the projects' names, then show that project's most recently shown workspace, or make a new workspace for it, as `workspace-new` does, when it has none. It SHALL ask nothing more.

#### Scenario: The project has workspaces
- **WHEN** project `p` has workspaces `p` and `p<2>`, `p<2>` was shown after `p`, another project's workspace is current, and `C-x p p p RET` is typed
- **THEN** `p<2>` is current and no workspace is made

#### Scenario: The project has none
- **WHEN** project `q` has no workspace and `C-x p p q RET` is typed
- **THEN** a new workspace `q` bound to `q` is current
