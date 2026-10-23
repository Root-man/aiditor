# workspaces Specification

## Purpose
Groups buffers into workspaces, each bound to one project, so that everything for one task can be created and killed together.

## Requirements

### Requirement: The editor starts with one workspace
At launch the editor SHALL have exactly one workspace, bound to a project, and it SHALL be current. The launch scratch buffer SHALL belong to it. No workspace SHALL ever be without a project.

#### Scenario: Launch
- **WHEN** the editor starts
- **THEN** there is one workspace, bound to a project, and its `*scratch*<name>` belongs to it

### Requirement: Every buffer belongs to one workspace
Every buffer SHALL belong to the workspace that was current when it was made, and keep that owner for its lifetime. `*Messages*` SHALL belong to no workspace.

#### Scenario: A file buffer is owned by the current workspace
- **WHEN** a new workspace is made and a file is visited in it
- **THEN** the file's buffer belongs to the new workspace

#### Scenario: Messages is global
- **WHEN** a message is logged while any workspace is current
- **THEN** `*Messages*` belongs to no workspace

### Requirement: A new workspace starts with a scratch buffer
Making a workspace with a name and a project SHALL bind it to that project and make it current. It SHALL get one new empty scratch buffer named `*scratch*<name>` after the workspace, shown in a single window that is selected. The workspace that was current SHALL keep its window configuration.

#### Scenario: New workspace
- **WHEN** a workspace `w` is made for project `p`
- **THEN** `w` is current and bound to `p`, and the frame shows one window, selected, on an empty `*scratch*<w>` owned by `w`

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
- **WHEN** only one workspace exists and it is killed
- **THEN** the kill fails with that message and the workspace and its buffers remain

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

### Requirement: A launch without an argument gets the home project
A launch that names no file or folder SHALL bind its workspace to an ad-hoc project whose primary is the home directory, named after it, whatever the working directory.

#### Scenario: No argument
- **WHEN** the editor starts with no argument
- **THEN** its workspace is bound to an ad-hoc project whose primary is `$HOME`, and no project file is written

### Requirement: A file or folder on the command line opens in the project that contains it
A file or folder named on the command line SHALL be bound to the first saved project one of whose roots contains it. With none, a file SHALL get an ad-hoc project at the nearest enclosing directory holding `.git`, or at its folder outside any repo, and a folder an ad-hoc project at itself, named after that directory. Nothing SHALL be written, and the `$HOME` project SHALL then not be listed.

#### Scenario: A file in a saved project
- **WHEN** a project `app` with an additional root `/r/lib` is saved and the editor starts with `/r/lib/x.rs`
- **THEN** the file's buffer is current in the only workspace, which is bound to `app`

#### Scenario: A file in a git repo
- **WHEN** no saved project contains `/r/repo/src/x.rs`, `/r/repo` holds `.git`, and the editor starts with that file
- **THEN** the workspace is bound to an ad-hoc project `repo` whose primary is `/r/repo`

#### Scenario: A file outside any repo
- **WHEN** no saved project contains `/r/notes/a.txt` and no ancestor holds `.git`
- **THEN** the workspace is bound to an ad-hoc project `notes` whose primary is `/r/notes`

#### Scenario: A folder
- **WHEN** no saved project contains `/r/new` and the editor starts with that folder
- **THEN** the workspace is `new`, bound to an ad-hoc project `new` whose primary is `/r/new`, showing `*scratch*<new>`, and no file is written

#### Scenario: A folder inside a saved project
- **WHEN** saved project `app` has a root `/r/lib` and the editor starts with `/r/lib/src`
- **THEN** the workspace is bound to `app` and no ad-hoc project is made

### Requirement: Workspace commands are keys and M-x commands
The editor SHALL provide `workspace-new`, `workspace-switch`, `workspace-next`, `workspace-previous`, `workspace-rename`, `workspace-kill`, `workspace-kill-current` and `workspace-list`, each runnable through `M-x` and bound under the `C-x t` prefix.

#### Scenario: Keys
- **WHEN** `C-x t` is pressed and the prefix hint shows
- **THEN** it lists a key for each of the eight workspace commands

### Requirement: New workspace prompts for a project
`workspace-new` (`C-x t 2`) SHALL prompt `New workspace for project: `, completing over the projects' names, then `Workspace name: `, and make a new current workspace bound to the chosen project. A blank name SHALL name it after the project, with `<2>`, `<3>` appended while another workspace has that name; a name another workspace has SHALL fail with `Workspace <name> already exists`. A name no project has SHALL fail with `No project named <name>`. A failure SHALL change nothing.

#### Scenario: Two workspaces on one project
- **WHEN** project `p` exists and `C-x t 2 p RET RET` is typed twice
- **THEN** workspaces `p` and `p<2>` exist, both bound to `p`, and `p<2>` is current, showing `*scratch*<p<2>>`

#### Scenario: A given name
- **WHEN** `C-x t 2 p RET mine RET` is typed
- **THEN** a workspace `mine` bound to `p` is current, showing `*scratch*<mine>`, and typing the same again says `Workspace mine already exists`

#### Scenario: Unknown project
- **WHEN** `workspace-new` is given a name no project has
- **THEN** no workspace is made and the echo area says `No project named <name>`

### Requirement: Switch to a workspace by name
`workspace-switch` (`C-x t RET`) SHALL prompt `Switch to workspace: `, completing over the workspaces' names, the others most recently shown first and the current one last, and show the chosen workspace. A name no workspace has SHALL fail with `No workspace named <name>`.

#### Scenario: RET alone goes back
- **WHEN** workspace `a` is shown, then `b`, and `C-x t RET RET` is typed
- **THEN** `a` is current

### Requirement: Next and previous workspace
`workspace-next` (`C-x t o`) SHALL show the workspace after the current one in creation order, wrapping from the last to the first, and `workspace-previous` (`C-x t O`) the one before. A prefix argument SHALL move that many, the other way when negative.

#### Scenario: Wrapping
- **WHEN** workspaces `a`, `b`, `c` exist in that order, `c` is current, and `C-x t o` is typed
- **THEN** `a` is current, and `C-x t O` makes `c` current again

#### Scenario: A count
- **WHEN** workspaces `a`, `b`, `c` exist, `a` is current, and `C-u 2 C-x t o` is typed
- **THEN** `c` is current

### Requirement: Rename the current workspace
`workspace-rename` (`C-x t r`) SHALL prompt `Rename workspace to: ` and rename the current workspace and its scratch buffer, which becomes `*scratch*<name>`. An empty name SHALL fail with `Empty workspace name`, and a name another workspace has SHALL fail with `Workspace <name> already exists`, changing nothing.

#### Scenario: Rename
- **WHEN** `C-x t r work RET` is typed
- **THEN** the current workspace is named `work` and its scratch buffer `*scratch*<work>`

#### Scenario: Taken name
- **WHEN** workspaces `a` and `b` exist, `b` is current, and it is renamed to `a`
- **THEN** the echo area says `Workspace a already exists` and the names are unchanged

### Requirement: Kill a workspace by name
`workspace-kill` (`C-x t k`) SHALL prompt `Kill workspace: `, completing over the workspaces' names with the current one first, and kill the chosen one. `workspace-kill-current` (`C-x t 0`) SHALL kill the current workspace without that prompt. Killing the sole workspace SHALL fail with `Attempt to delete the sole workspace` before any other question.

#### Scenario: Kill the current one
- **WHEN** workspaces `a` and `b` exist, `b` is current with no modified file buffer, and `C-x t 0` is typed
- **THEN** `b` is killed without a question and `a` is current

#### Scenario: Sole workspace
- **WHEN** only one workspace exists and `C-x t 0` or `C-x t k RET` is typed
- **THEN** the echo area says `Attempt to delete the sole workspace` and nothing else is asked

### Requirement: Killing a workspace asks about its modified file buffers
Both kill commands SHALL ask `Save file <path>? ` for each modified file buffer of that workspace, as quitting does, and, when any was declined, `Modified buffers exist; kill anyway? ` over `No` then `Yes`. The workspace SHALL be killed once every chosen save has succeeded; a declined `kill anyway`, or a failed save, SHALL leave it.

#### Scenario: Saving then killing
- **WHEN** the current workspace `b` has a modified file buffer and `C-x t 0 RET` is typed
- **THEN** the prompt was `Save file <path>? `, the file is written and `b` is killed

#### Scenario: Declining keeps the workspace
- **WHEN** `b` has a modified file buffer and `C-x t 0 no RET RET` is typed
- **THEN** `b` and its modified buffer remain

#### Scenario: Kill anyway
- **WHEN** `b` has a modified file buffer and `C-x t k RET no RET yes RET` is typed
- **THEN** `b` is killed and the file is unchanged on disk

#### Scenario: Buffers of other workspaces are not asked about
- **WHEN** only workspace `a` has a modified file buffer and `b` is killed
- **THEN** no save question is asked

### Requirement: List the workspaces
`workspace-list` (`C-x t l`) SHALL show every workspace in creation order in the echo area, each as `<name> (<project>)`, the current one marked with a leading `*`, separated by two spaces.

#### Scenario: Two workspaces
- **WHEN** workspaces `a` on `p` and `b` on `q` exist, `b` is current, and `C-x t l` is typed
- **THEN** the echo area shows `a (p)  *b (q)`

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
