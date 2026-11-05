## RENAMED Requirements

- FROM: `### Requirement: The editor starts with one workspace`
- TO: `### Requirement: The editor starts with at most one workspace`

- FROM: `### Requirement: The last workspace cannot be killed`
- TO: `### Requirement: Killing the last workspace shows the lobby`

- FROM: `### Requirement: A launch without an argument gets the home project`
- TO: `### Requirement: A launch without an argument opens the lobby`

## MODIFIED Requirements

### Requirement: The editor starts with at most one workspace
A launch that names a file or folder SHALL start with exactly one workspace, bound to a project, and it SHALL be current; its scratch buffer SHALL belong to it. A launch that names neither SHALL start with no workspace, in the lobby. No workspace SHALL ever be without a project.

#### Scenario: Launch
- **WHEN** the editor starts with a folder
- **THEN** there is one workspace, bound to a project, and its `*scratch*<name>` belongs to it

#### Scenario: Launch without an argument
- **WHEN** the editor starts with no argument
- **THEN** there is no workspace and no scratch buffer, and the frame shows the lobby list in one selected window

### Requirement: Every buffer belongs to one workspace
Every buffer made while a workspace is current SHALL belong to that workspace, and keep that owner for its lifetime. `*Messages*` and the lobby's `*Workspaces*` SHALL belong to no workspace.

#### Scenario: A file buffer is owned by the current workspace
- **WHEN** a new workspace is made and a file is visited in it
- **THEN** the file's buffer belongs to the new workspace

#### Scenario: Messages is global
- **WHEN** a message is logged while any workspace is current, or in the lobby
- **THEN** `*Messages*` belongs to no workspace

### Requirement: Killing the last workspace shows the lobby
Killing the only workspace SHALL kill it as any other workspace is killed, after the same save and process questions, and then show the lobby.

#### Scenario: Sole workspace
- **WHEN** only one workspace exists, it has no modified file buffer or live process, and it is killed
- **THEN** it and its buffers are gone, there is no workspace, and the frame shows the lobby list in one selected window

#### Scenario: Declining keeps the sole workspace
- **WHEN** the only workspace has a modified file buffer and `C-x t 0 no RET RET` is typed
- **THEN** the workspace and its modified buffer remain and the lobby is not shown

### Requirement: A launch without an argument opens the lobby
A launch that names no file or folder SHALL make no project for the home directory and no workspace, whatever the working directory. The lobby list SHALL show only the saved projects.

#### Scenario: No argument
- **WHEN** the editor starts with no argument and project `app` is saved
- **THEN** no workspace exists, the lobby list shows `app` with `no workspaces`, no `$HOME` project is listed, and no project file is written

### Requirement: Kill a workspace by name
`workspace-kill` (`C-x t k`) SHALL prompt `Kill workspace: `, completing over the workspaces' names with the current one first, and kill the chosen one. `workspace-kill-current` (`C-x t 0`) SHALL kill the current workspace without that prompt. Killing the sole workspace SHALL show the lobby once it is killed.

#### Scenario: Kill the current one
- **WHEN** workspaces `a` and `b` exist, `b` is current with no modified file buffer, and `C-x t 0` is typed
- **THEN** `b` is killed without a question and `a` is current

#### Scenario: Sole workspace
- **WHEN** only one workspace exists with no modified file buffer or live process, and `C-x t 0` or `C-x t k RET` is typed
- **THEN** nothing is asked, the workspace is killed and the lobby list is shown

### Requirement: Workspace list keys live in the echo line
The workspace list SHALL start with its first project row, without a header or leading blank row. While the selected window shows a workspace list and no message or minibuffer is up, the echo line SHALL show `n/p move   RET switch   TAB fold   g refresh   q quit`. Keys SHALL use bold text in the `workspace-current` foreground and action labels SHALL use `mode-line-dim`, as frames 13–13d and 6d. These texts are the bundled keymap's: each hint SHALL show the shortest key sequence now bound to its command, preferring plain characters among equally short ones, `/`-joined for a pair, and an action with no bound key SHALL be omitted. Selecting another window SHALL remove these hints even if the list remains visible elsewhere. A message SHALL replace the hints until the next key; timers SHALL retain the message. An open minibuffer SHALL retain its ordinary prompt. Hints SHALL wrap between complete actions into a second row when they do not fit, and the windows SHALL yield the extra echo height.

#### Scenario: Context follows the selected window
- **GIVEN** a split with the workspace list selected and an ordinary buffer in the other window
- **WHEN** the list opens and selection then moves to the other window and back
- **THEN** the first buffer row is a project, the hints are shown for the selected list, disappear for the ordinary buffer and return for the list

#### Scenario: A message replaces hints for one key
- **GIVEN** point is on an informational buffer row in the workspace list
- **WHEN** RET is pressed, a timer fires, and then n is pressed
- **THEN** the echo line shows `Select a project or workspace row` through the timer, and returns to the hints after n

#### Scenario: A prompt has precedence
- **GIVEN** the selected window shows the workspace list hints
- **WHEN** M-x opens and then its prompt is accepted to reopen the list
- **THEN** the ordinary minibuffer prompt replaces the hints while open, and the hints return when the list is selected again

#### Scenario: Hints wrap and windows yield height
- **GIVEN** a frame wide enough for all five action hints on one row
- **WHEN** the frame narrows enough to fit the hints on two rows and later widens again
- **THEN** complete hints wrap to the second row with their key styles retained, every window yields one extra echo row, and widening restores the original echo height

#### Scenario: Hints follow the keymap
- **GIVEN** a user keymap binds `project-switch` to `C-c p p` and `C-x p p` and `C-x C-f` to other commands
- **WHEN** the lobby list is shown
- **THEN** the echo line shows `C-c p p project` and no `find file` hint

## ADDED Requirements

### Requirement: The lobby has no workspace
The lobby SHALL be the state with no workspace. It SHALL show a read-only, fileless `*Workspaces*` list owned by no workspace, in a single selected window. The workspace modeline SHALL show no workspace, and the mode line SHALL show no project or `[unwatched]` label. Leaving the lobby by making a workspace SHALL kill the lobby list and show the new workspace's single window. Returning to the lobby SHALL show a fresh list. `*Messages*` SHALL be kept in both directions.

#### Scenario: Leaving the lobby
- **GIVEN** the lobby and a saved project `app`
- **WHEN** `C-x p p app RET` is typed
- **THEN** a workspace on `app` is current, showing its `*scratch*` in one window, and no `*Workspaces*` buffer exists

#### Scenario: Messages survives the lobby
- **GIVEN** a message was logged in the lobby
- **WHEN** a workspace is made and later killed as the last one
- **THEN** `*Messages*` holds the message both times

### Requirement: The lobby list
In the lobby, the `*Workspaces*` list SHALL use the workspace list's rows, keys and refresh. It SHALL put point on the first project row. `RET` on a project row SHALL make that project's first workspace and show it. `q` and `C-g` SHALL keep the list shown. With no projects, the list SHALL show the single line `No projects` and its mode line SHALL show `*Workspaces* │ 0 projects, 0 workspaces` without a position. While the lobby list is selected and no message or minibuffer is up, the echo line SHALL show `n/p move   RET switch   TAB fold   g refresh   C-x C-f find file   C-x p p project` with the bundled keymap, its keys following the keymap, styled and wrapped as the workspace list's hints.

#### Scenario: RET on a project
- **GIVEN** the lobby lists saved projects `alpha` and `beta`
- **WHEN** `n RET` is typed
- **THEN** a workspace on `beta` is current with its scratch buffer, and the lobby list is gone

#### Scenario: Quit keeps the lobby
- **GIVEN** the lobby
- **WHEN** `q` or `C-g` is typed
- **THEN** the lobby list is still shown and no workspace exists

#### Scenario: No projects
- **GIVEN** no project is saved
- **WHEN** the editor starts with no argument
- **THEN** the list shows `No projects`, the mode line shows `*Workspaces* │ 0 projects, 0 workspaces`, and the echo line names `C-x C-f` and `C-x p p`

### Requirement: Commands that make a workspace from the lobby
In the lobby, these commands SHALL make a workspace:
- `find-file` SHALL start its prompt in the home directory. Visiting a file SHALL make a workspace for the project the file belongs to, chosen as for a file named on the command line, and show the file's buffer in it. A failed visit SHALL echo its error and stay in the lobby without making a project.
- `project-switch` SHALL make a workspace for the chosen project.
- `workspace-new` SHALL make a workspace for the chosen project, with the first project selected initially.
- Opening a recovered file SHALL make its workspace as `find-file` does.

#### Scenario: find-file in a git repo
- **GIVEN** the lobby, no saved project contains `/r/repo/src/x.rs`, and `/r/repo` holds `.git`
- **WHEN** `C-x C-f` visits that file
- **THEN** a workspace `repo` on an ad-hoc project `repo` is current, showing the file's buffer, and the lobby list is gone

#### Scenario: find-file in a saved project
- **GIVEN** the lobby and a saved project `app` with root `/r/app`
- **WHEN** `C-x C-f` visits `/r/app/a.txt`
- **THEN** a workspace on `app` is current, showing `a.txt`

#### Scenario: find-file starts at home
- **GIVEN** the lobby
- **WHEN** `C-x C-f` is typed
- **THEN** the prompt lists the home directory

#### Scenario: A failed visit stays in the lobby
- **GIVEN** the lobby
- **WHEN** `C-x C-f` visits a file that cannot be read
- **THEN** the error is echoed, no workspace or project is made and the lobby list is shown

### Requirement: Commands that need a workspace fail in the lobby
In the lobby, every command that acts on the current workspace or its project SHALL fail with `No workspace` and change nothing. This includes `project-find-file`, `project-grep`, `vterm`, `agent-start`, `agent-sessions`, `workspace-switch`, `workspace-next`, `workspace-previous`, `workspace-rename`, `workspace-kill` and `workspace-kill-current`. `workspace-list` SHALL refresh the lobby list.

#### Scenario: Grep in the lobby
- **GIVEN** the lobby
- **WHEN** `project-grep` is run through `M-x`
- **THEN** the echo area says `No workspace` and the lobby list is still shown

#### Scenario: Terminal in the lobby
- **GIVEN** the lobby
- **WHEN** `M-x vterm RET` is typed
- **THEN** the echo area says `No workspace` and no terminal is started
