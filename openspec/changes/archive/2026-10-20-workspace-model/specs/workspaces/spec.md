# Spec Delta

## Purpose

Groups buffers into workspaces, each bound to one project, so that everything for one task can be created and killed together.

## ADDED Requirements

### Requirement: The editor starts with one workspace
At launch the editor SHALL have exactly one workspace, named `default` and bound to no project, and it SHALL be current. The launch `*scratch*` buffer SHALL belong to it.

#### Scenario: Launch
- **WHEN** the editor starts
- **THEN** there is one workspace, `default`, with no project, and `*scratch*` belongs to it

### Requirement: Every buffer belongs to one workspace
Every buffer SHALL belong to the workspace that was current when it was made, and keep that owner for its lifetime. `*Messages*` SHALL belong to no workspace.

#### Scenario: A file buffer is owned by the current workspace
- **WHEN** a new workspace is made and a file is visited in it
- **THEN** the file's buffer belongs to the new workspace

#### Scenario: Messages is global
- **WHEN** a message is logged while any workspace is current
- **THEN** `*Messages*` belongs to no workspace

### Requirement: A new workspace starts with a scratch buffer
Making a workspace with a name and a project SHALL bind it to that project, make it current, give it one new empty scratch buffer, and show that buffer in the selected window.

#### Scenario: New workspace
- **WHEN** a workspace `w` is made for project `p`
- **THEN** `w` is current and bound to `p`, and the selected window shows an empty scratch buffer owned by `w`

### Requirement: Killing a workspace kills its buffers
Killing a workspace SHALL kill every buffer it owns, modified or not, and SHALL leave every other workspace and its buffers, and `*Messages*`, as they were. Windows that showed a killed buffer SHALL show a buffer of the current workspace instead, or `*Messages*`.

#### Scenario: Kill a workspace that is not current
- **WHEN** workspaces `a` and `b` each have buffers, `b` is current and `a` is killed
- **THEN** `a` and its buffers are gone, and `b`, its buffers and `*Messages*` are unchanged

#### Scenario: Kill the current workspace
- **WHEN** workspaces `a` and `b` exist, `b` is current and `b` is killed
- **THEN** `b` and its buffers are gone, `a` is current, and the selected window shows a buffer of `a`

### Requirement: The last workspace cannot be killed
Killing the only workspace SHALL fail with `Attempt to delete the sole workspace` and change nothing.

#### Scenario: Sole workspace
- **WHEN** only `default` exists and it is killed
- **THEN** the kill fails with that message and `default` and its buffers remain
