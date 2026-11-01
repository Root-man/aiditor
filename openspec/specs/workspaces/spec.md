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
`workspace-new` (`C-x t 2`) SHALL prompt `New workspace for project: `, completing over the projects' names with the active workspace's project selected initially, then `Workspace name: `, and make a new current workspace bound to the chosen project. Pressing `RET` without changing the initial project selection SHALL choose the active workspace's actual project, including when another project has the same display name. The user SHALL still be able to select a different project. A blank name SHALL name the workspace after the project, with `<2>`, `<3>` appended while another workspace has that name; a name another workspace has SHALL fail with `Workspace <name> already exists`. A name no project has SHALL fail with `No project named <name>`. A failure SHALL change nothing.

#### Scenario: Two workspaces on one project
- **GIVEN** project `p` exists
- **WHEN** `C-x t 2 p RET RET` is typed twice
- **THEN** workspaces `p` and `p<2>` exist, both bound to `p`, and `p<2>` is current, showing `*scratch*<p<2>>`

#### Scenario: A given name
- **GIVEN** project `p` exists and no workspace is named `mine`
- **WHEN** `C-x t 2 p RET mine RET` is typed
- **THEN** a workspace `mine` bound to `p` is current, showing `*scratch*<mine>`, and typing the same again says `Workspace mine already exists`

#### Scenario: Unknown project
- **GIVEN** no project has the supplied name
- **WHEN** `workspace-new` is given that name
- **THEN** no workspace is made and the echo area says `No project named <name>`

#### Scenario: Default to the active project
- **GIVEN** the active workspace uses project `zeta` and `alpha` appears earlier in the project list
- **WHEN** `C-x t 2 RET task RET` is typed
- **THEN** the project prompt initially selects `zeta` and the new workspace `task` is bound to `zeta`

#### Scenario: Select another project
- **GIVEN** the active workspace uses project `zeta` and project `alpha` also exists
- **WHEN** the user opens `workspace-new`, selects `alpha`, and accepts a workspace name
- **THEN** the new workspace is bound to `alpha`

#### Scenario: Duplicate names do not redirect the default
- **GIVEN** the active workspace uses the second of two projects named `app`, with different primary roots
- **WHEN** the user accepts the initial project selection and enters a new workspace name
- **THEN** the new workspace uses the same project and roots as the originating workspace

#### Scenario: Cancel creation
- **GIVEN** an existing workspace is active
- **WHEN** the user cancels either workspace-new prompt with `C-g`
- **THEN** the existing workspace stays active with its buffers and layout intact

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
`workspace-list` (`C-x t l`, also through `M-x`) SHALL open and select a temporary read-only `*Workspaces*` buffer in the selected window, showing every live workspace in a project → workspace → buffers tree. Projects SHALL be grouped by identity, in order of their first workspace's creation, with workspaces in creation order within each group. Each project heading SHALL show its name; equal project names SHALL be distinguished by their primary root. Each workspace row SHALL show its name, with the current workspace marked `*`. Beneath each workspace, the tree SHALL show its listed owned buffers, including scratch and file buffers, sorted by buffer name. Internal hidden buffers, workspace-list buffers themselves, and the global `*Messages*` SHALL be omitted from the owned-buffer branches. Projects with no workspace SHALL be omitted.

The list SHALL be an on-demand snapshot, refreshed whenever `workspace-list` is run. It SHALL reuse one list buffer per invoking workspace, with normal buffer-name disambiguation, and SHALL be read-only, fileless and unmodified. Opening or refreshing it SHALL retain all user buffer contents and the existing split structure. It SHALL initially place selection on the current workspace's row. It SHALL use the ordinary buffer display, scrolling and read-only behavior.

#### Scenario: Two workspaces
- **GIVEN** workspaces `a` on `p` and `b` on `q` exist and `b` is current
- **WHEN** `C-x t l` is typed
- **THEN** the selected list buffer shows `a` under `p` and `*b` under `q`, with their owned buffers beneath them

#### Scenario: Project, workspace and buffer hierarchy
- **GIVEN** workspaces `a` and `b` belong to project `p`, workspace `c` belongs to project `q`, each has a scratch buffer, and `b` also owns `notes.txt`
- **WHEN** `b` runs `C-x t l`
- **THEN** the selected read-only buffer shows project `p` with children `a` and `*b`, `notes.txt` under `b`, project `q` with child `c`, and each workspace's scratch beneath its owner

#### Scenario: Refresh an existing list
- **GIVEN** a workspace has opened its list and a buffer has since been opened or killed
- **WHEN** `workspace-list` runs again in that workspace
- **THEN** the same list buffer shows the current workspace and buffer inventory

#### Scenario: Separate projects with equal names
- **GIVEN** two distinct projects named `app` have different primary roots and each has a workspace
- **WHEN** the list is opened
- **THEN** separate `app` headings include their primary roots and each workspace appears under its actual project

#### Scenario: Buffer ownership remains clear
- **GIVEN** two workspaces visit the same file and `*Messages*` and a hidden preview buffer exist
- **WHEN** the list is opened
- **THEN** each file buffer appears beneath its owner alongside that workspace's scratch, and the owned-buffer branches contain only listed workspace buffers other than the list itself

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

### Requirement: Watch labels apply only to file-backed buffers
An ordinary window displaying a buffer without an associated file SHALL omit the project's `[unwatched]` mode-line label. This includes scratch, `*Messages*`, temporary workspace lists and other fileless buffers. Windows displaying file-backed buffers SHALL retain the existing project eligibility label, including disabled watching, exceeded watch limits and pending eligibility counts. Each window SHALL use its own displayed buffer's file association. This display rule narrows the project watch label's scope without changing project watch eligibility or visited-file watching.

#### Scenario: Scratch and a file share a split
- **GIVEN** an unwatched project's workspace displays scratch and a file-backed buffer in separate windows
- **WHEN** the frame is rendered
- **THEN** scratch has its ordinary mode line and the file-backed window includes `[unwatched]`, regardless of which window is selected

#### Scenario: Temporary and global fileless buffers
- **GIVEN** the active workspace's project is unwatched
- **WHEN** a window displays `*Workspaces*` or `*Messages*`
- **THEN** it shows the buffer's ordinary mode line without the project watch label

#### Scenario: File-backed labels follow eligibility
- **GIVEN** a file-backed buffer is displayed in an unwatched project's workspace
- **WHEN** the project becomes eligible for root watching
- **THEN** the mode line returns to its ordinary form, and `[unwatched]` appears again if eligibility is lost

### Requirement: Only workspace rows activate in the workspace list
In the workspace list, `n`/`p`, `C-n`/`C-p` and the down/up arrows SHALL move selection among workspace rows, skipping project headings and buffer rows and stopping at the first or last workspace. `RET` on a workspace row SHALL dismiss the invoking list and switch to that workspace using its existing window configuration. Choosing the current workspace SHALL simply dismiss the list. Project and buffer rows SHALL be informational: `RET` on either SHALL keep the current workspace and list visible and report `Select a workspace row`. Navigation from such a row SHALL select the next or previous workspace in the requested direction, clamping to an end workspace when necessary. `q` or `C-g` SHALL dismiss the list and return to an ordinary buffer in the invoking workspace. Dismissal SHALL use normal buffer-kill replacement behavior. The list SHALL NOT provide project activation, individual buffer activation or destructive workspace actions.

#### Scenario: Navigate only workspaces
- **GIVEN** a tree with several project headings, workspace rows and buffer children
- **WHEN** the user presses `n`, `p`, `C-n`, `C-p` or the arrow keys
- **THEN** selection moves between workspace rows, skipping informational rows and stopping at either end

#### Scenario: Enter switches the entire workspace
- **GIVEN** workspace `a` shows the list and workspace `b` has a saved two-window layout
- **WHEN** the user selects `b` and presses `RET`
- **THEN** `b` becomes active with its saved layout, selected buffer and point, and the invoking list has been dismissed

#### Scenario: An informational row cannot activate
- **GIVEN** ordinary point movement has placed point on a project or buffer row
- **WHEN** the user presses `RET`
- **THEN** the current workspace and list stay visible and the echo area says `Select a workspace row`

#### Scenario: Dismiss or choose the current workspace
- **GIVEN** the current workspace shows its list with an ordinary buffer available
- **WHEN** the user presses `q`, `C-g`, or `RET` on the current workspace row
- **THEN** the list is dismissed, an ordinary buffer is shown using normal buffer replacement, and the current workspace and split structure are retained

#### Scenario: A workspace disappeared after the snapshot
- **GIVEN** a list contains a workspace that has since been killed
- **WHEN** that stale row is activated
- **THEN** the editor reports `No such workspace` and leaves the current workspace and list available

### Requirement: Killing a workspace asks about its live processes
After the save questions, when a buffer of that workspace is a live terminal or a session that hasn't ended, both kill commands SHALL ask `Active processes exist; kill them and the workspace anyway? ` over `No` then `Yes`. `Yes` SHALL kill the workspace; any other answer SHALL leave it, still saving the buffers answered `Yes`.

#### Scenario: A live terminal
- **WHEN** workspace `b` holds a live terminal and `C-x t 0 yes RET` is typed
- **THEN** the prompt was `Active processes exist; kill them and the workspace anyway? ` and `b` is killed

#### Scenario: Declining keeps the workspace
- **WHEN** workspace `b` holds a live session and the question is answered `No`
- **THEN** `b`, its buffer and the agent remain

#### Scenario: Saves still happen after declining
- **WHEN** `b` has a modified file buffer and a live terminal, the save question is answered `Yes` and the process question `No`
- **THEN** the file is written and `b` remains

#### Scenario: Processes of other workspaces are not asked about
- **WHEN** only workspace `a` holds a live terminal and `b` is killed
- **THEN** no process question is asked

#### Scenario: An ended session
- **WHEN** `b` holds only a session whose agent has exited and `b` is killed
- **THEN** no process question is asked
