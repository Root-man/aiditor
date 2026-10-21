# workspaces Specification

## Purpose
Groups buffers into workspaces, each bound to one project, so that everything for one task can be created and killed together.

## Requirements

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
Making a workspace with a name and a project SHALL bind it to that project and make it current. It SHALL get one new empty scratch buffer, shown in a single window that is selected. The workspace that was current SHALL keep its window configuration.

#### Scenario: New workspace
- **WHEN** a workspace `w` is made for project `p`
- **THEN** `w` is current and bound to `p`, and the frame shows one window, selected, on an empty scratch buffer owned by `w`

#### Scenario: The previous workspace keeps its layout
- **WHEN** a workspace is split into two windows, a new workspace is made, and the first is switched back to
- **THEN** the first workspace shows its two windows again

### Requirement: Killing a workspace kills its buffers
Killing a workspace SHALL kill every buffer it owns, modified or not, and drop its window configuration. Every other workspace, its buffers and its window configuration, and `*Messages*`, SHALL stay as they were. Killing the current workspace SHALL show the window configuration of the workspace that becomes current. A window of another workspace that showed a killed buffer SHALL show a buffer of its own workspace instead, or `*Messages*`.

#### Scenario: Kill a workspace that is not current
- **WHEN** workspaces `a` and `b` each have buffers, `b` is current and `a` is killed
- **THEN** `a` and its buffers are gone, and `b`, its buffers and `*Messages*` are unchanged

#### Scenario: Kill the current workspace
- **WHEN** workspaces `a` and `b` exist, `a` has a split, `b` is current and `b` is killed
- **THEN** `b` and its buffers are gone, `a` is current, and the frame shows `a`'s split as it was

### Requirement: The last workspace cannot be killed
Killing the only workspace SHALL fail with `Attempt to delete the sole workspace` and change nothing.

#### Scenario: Sole workspace
- **WHEN** only `default` exists and it is killed
- **THEN** the kill fails with that message and `default` and its buffers remain

### Requirement: Each workspace keeps its own window configuration
Each workspace SHALL have its own window configuration: the split tree, each window's buffer, point, mark and scroll position, and the selected window. Switching to another workspace SHALL show that workspace's configuration as it was when it was last current. Changes made to windows while one workspace is current SHALL NOT change another workspace's configuration.

#### Scenario: Switching away and back restores the layout
- **WHEN** a workspace is split into two windows on different buffers with points moved, another workspace is switched to, and the first is switched back to
- **THEN** the first workspace shows the same two windows in the same split, each on its buffer at its point, with the same window selected

#### Scenario: Another workspace's splits are its own
- **WHEN** workspace `a` has two windows and workspace `b` three, and `C-x 1` is typed in `a`
- **THEN** switching to `b` shows its three windows, and switching back to `a` shows its one window

#### Scenario: Points follow edits made while away
- **WHEN** a workspace's window shows a buffer with point after some text, another workspace is switched to, and text is inserted before that point while the other workspace is current
- **THEN** switching back shows point after the same text, moved by the insertion

### Requirement: Switching closes the minibuffer
Switching to another workspace while the minibuffer is open SHALL close it first, cancelling its prompt. Switching to the workspace that is already current SHALL change nothing.

#### Scenario: Switch with a prompt open
- **WHEN** a prompt is open in the minibuffer and another workspace is switched to
- **THEN** the minibuffer is closed and the other workspace's selected window is selected

### Requirement: A killed buffer is replaced in every workspace's windows
Killing a buffer SHALL replace it in the windows of every workspace that shows it, not only the current one. In each workspace, the replacement SHALL be that workspace's most recent other listed buffer, or `*Messages*`.

#### Scenario: A window of a workspace that is not current
- **WHEN** a window of workspace `a` shows a buffer of workspace `b`, `b` is current, and that buffer is killed
- **THEN** switching to `a` shows a buffer of `a` or `*Messages*` in that window
