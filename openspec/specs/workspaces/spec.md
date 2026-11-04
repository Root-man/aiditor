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
`workspace-list` (`C-x t l`, also through `M-x`) SHALL open and select a temporary read-only `*Workspaces*` buffer in the selected window, showing every project and live workspace in a project → workspace → buffers tree. Projects SHALL be grouped by identity, in order of their first workspace's creation, with workspaces in creation order within each group. Each project heading SHALL show its name and primary root, including when names are unique. Each workspace row SHALL show its name, with the current workspace marked `*`. Beneath each workspace, the tree SHALL show its listed owned buffers, including scratch and file buffers, sorted by buffer name. Internal hidden buffers, workspace-list buffers themselves, and the global `*Messages*` SHALL be omitted from the owned-buffer branches. Projects with no workspace SHALL follow populated projects, sorted by name, and show `no workspaces`.

The list SHALL be an on-demand snapshot, refreshed whenever `workspace-list` is run. It SHALL reuse one list buffer per invoking workspace, with normal buffer-name disambiguation, and SHALL be read-only, fileless and unmodified. Opening or refreshing it SHALL retain all user buffer contents and the existing split structure. It SHALL initially place selection on the current workspace's row, or its project row if that project remains folded on refresh. It SHALL use the ordinary buffer display, scrolling and read-only behavior.

#### Scenario: Two workspaces
- **GIVEN** workspaces `a` on `p` and `b` on `q` exist and `b` is current
- **WHEN** `C-x t l` is typed
- **THEN** the selected list buffer shows `a` under `p` and `*b` under `q`, with the current workspace's owned buffers beneath it and the other workspace folded

#### Scenario: Project, workspace and buffer hierarchy
- **GIVEN** workspaces `a` and `b` belong to project `p`, workspace `c` belongs to project `q`, each has a scratch buffer, and `b` also owns `notes.txt`
- **WHEN** `b` runs `C-x t l`
- **THEN** the selected read-only buffer shows project `p` with children `a` and `*b`, `notes.txt` under `b`, project `q` with child `c`, and the current workspace's scratch beneath its owner

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
- **WHEN** the list is opened and both workspaces are unfolded
- **THEN** each file buffer appears beneath its owner alongside that workspace's scratch, and the owned-buffer branches contain only listed workspace buffers other than the list itself

#### Scenario: Empty projects follow populated projects
- **GIVEN** projects `zeta` and `alpha` have no workspace and project `p` has one
- **WHEN** the list is opened
- **THEN** `p` appears first, followed by `alpha` and `zeta`, each showing its primary root and `no workspaces`

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
In the workspace list, `n`/`p`, `C-n`/`C-p` and the down/up arrows SHALL move selection among visible project and workspace rows, skipping hidden children, blank and buffer rows and stopping at the first or last selectable row. `RET` on a workspace row SHALL dismiss the invoking list and switch to that workspace using its existing window configuration. Choosing the current workspace SHALL simply dismiss the list. `RET` on a project row SHALL dismiss the list and run project-switch semantics for that project identity: show its most recently shown workspace, or create one when none exists. Buffer rows SHALL be informational: `RET` on one SHALL keep the current workspace and list visible and report `Select a project or workspace row`. Navigation from such a row SHALL select the next or previous selectable row in the requested direction, clamping to an end row when necessary. `q` or `C-g` SHALL dismiss the list and return to an ordinary buffer in the invoking workspace. Dismissal SHALL use normal buffer-kill replacement behavior. The list SHALL NOT provide individual buffer activation or destructive workspace actions.

#### Scenario: Navigate only workspaces
- **GIVEN** two adjacent workspace rows within one project, separated by their buffer children
- **WHEN** the user moves forward from the first workspace or backward from the second with `n`/`p`, `C-n`/`C-p` or arrows
- **THEN** selection moves directly to the adjacent workspace, skipping its informational buffer rows

#### Scenario: Enter switches the entire workspace
- **GIVEN** workspace `a` shows the list and workspace `b` has a saved two-window layout
- **WHEN** the user selects `b` and presses `RET`
- **THEN** `b` becomes active with its saved layout, selected buffer and point, and the invoking list has been dismissed

#### Scenario: An informational row cannot activate
- **GIVEN** ordinary point movement has placed point on a buffer row
- **WHEN** the user presses `RET`
- **THEN** the current workspace and list stay visible and the echo area says `Select a project or workspace row`

#### Scenario: Dismiss or choose the current workspace
- **GIVEN** the current workspace shows its list with an ordinary buffer available
- **WHEN** the user presses `q`, `C-g`, or `RET` on the current workspace row
- **THEN** the list is dismissed, an ordinary buffer is shown using normal buffer replacement, and the current workspace and split structure are retained

#### Scenario: A workspace disappeared after the snapshot
- **GIVEN** a list contains a workspace that has since been killed
- **WHEN** that stale row is activated
- **THEN** the editor reports `No such workspace` and leaves the current workspace and list available

#### Scenario: Navigate projects and workspaces
- **GIVEN** a tree with several project headings, workspace rows and buffer children
- **WHEN** the user presses `n`, `p`, `C-n`, `C-p` or the arrow keys
- **THEN** selection moves between visible project and workspace rows, skipping hidden children, blank and buffer rows and stopping at either end

#### Scenario: Project activation reuses its most recent workspace
- **GIVEN** project `p` has workspaces `a` and `b`, `a` was most recently shown, and another project is current
- **WHEN** the user selects project `p` and presses `RET`
- **THEN** `a` is shown with its saved layout and the invoking list is dismissed

#### Scenario: Project activation creates its first workspace
- **GIVEN** a listed project has no workspace
- **WHEN** the user selects that project and presses `RET`
- **THEN** a new workspace bound to that project is shown with its scratch buffer and the invoking list is dismissed

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

### Requirement: Workspace list mode line counts selectable rows
The workspace list mode line SHALL show `*Workspaces* │ N projects, M workspaces` and a one-based selectable-row position such as `2/8`. Counts SHALL describe the full list snapshot, including empty projects and hidden workspaces. The position denominator SHALL count visible project and workspace rows only. On informational lines, position SHALL use the preceding selectable row, clamped to the first row before any selectable row.

#### Scenario: Counts include empty projects
- **GIVEN** the list snapshot contains five projects, including one empty project, and four workspaces
- **WHEN** all projects are open and point is on the second selectable row
- **THEN** the mode line shows `*Workspaces* │ 5 projects, 4 workspaces` and `2/9`

#### Scenario: Refresh updates totals
- **GIVEN** an existing list snapshot and a newly registered empty project
- **WHEN** the list is refreshed
- **THEN** the project count and selectable-row denominator increase by one while the workspace count stays the same

### Requirement: Workspace list rows show location and state
The workspace list SHALL lay out each snapshot row in name, where and state columns, using frame 13/13b's existing named faces in either theme. Project names SHALL be bold default text and primary roots SHALL use annotation. Empty projects SHALL include `no workspaces` beside their root. Workspace names SHALL use default text, with the current mark and name in bold workspace-current. Each workspace's where column SHALL count its listed owned buffers and, when nonzero, its modified file buffers as unsaved. Its state SHALL show its most urgent agent activity, with exactly the workspace modeline's priority and status face and the session mode line's word.

Buffer names SHALL use mode-line-dim, omit their owning `<workspace>` suffix when present, and remain informational. Their where columns SHALL use annotation: file directories as in their mode lines (relative to their own project's roots), terminal directories as in terminal mode lines, and session model and effort labels as in session mode lines. Other fileless buffers SHALL have an empty where column. Buffer state SHALL use the corresponding existing status face and words: `● unsaved`, `⊘ read-only`, `[deleted]`, `[conflict]`, `◉ command`, `exit n`, `● working`, `● awaiting permission`, or `● exited` when applicable; clean/idle buffers SHALL have no state. Multiple applicable file states SHALL remain visible. Control characters in row values SHALL be rendered safely on one line. The list SHALL remain an on-demand snapshot until refreshed.

#### Scenario: Workspace counts and urgent agent state
- **GIVEN** a workspace has six listed buffers, one modified file, a working session and a session awaiting permission
- **WHEN** the list is refreshed
- **THEN** its where column says `6 buffers · 1 unsaved` and its state says `● awaiting permission` in the same face as its workspace modeline dot

#### Scenario: Buffer kinds retain their own locations and states
- **GIVEN** file buffers are modified, read-only or deleted, a terminal has a command or failed exit, and a session has model/effort labels and activity
- **WHEN** the list is opened
- **THEN** each buffer row shows its own directory or model/effort, its applicable mode-line state words and faces, and its name without its owning workspace suffix

#### Scenario: Safe snapshot values
- **GIVEN** project, workspace, buffer or location values contain control characters
- **WHEN** the list is opened and the underlying state later changes
- **THEN** each value stays within its original row and the snapshot text/states stay unchanged until the command runs again

#### Scenario: Both themes resolve existing faces
- **GIVEN** a list contains project, current/other workspace, file, terminal and session rows
- **WHEN** rendered under dark or light theme
- **THEN** project names are bold, buffer names are dim, locations use annotation and each state uses its existing status face

### Requirement: Workspace list point sits on selectable names
Opening or refreshing the list SHALL put point on the first character of the current workspace's name, or its project name when that project remains folded on refresh. Moving between selectable project/workspace rows SHALL put point on the first character of that row's name, after indentation and the current marker. The selectable row at point SHALL use completion-selected background independently of the current workspace marker.

#### Scenario: Point follows names
- **GIVEN** a current workspace and another project/workspace are listed
- **WHEN** the list opens and the user moves with any workspace-list navigation binding
- **THEN** point starts on the selected name and that row has completion-selected background while the current workspace keeps its workspace-current face

### Requirement: Workspace list folds preserve snapshot presentation
TAB SHALL fold or unfold the project or workspace row at point, retaining point on that row's name. A row with children SHALL show `▾` when open and `▸` when folded in session-thought, with indentation rather than tree connectors. Rows without children SHALL have no fold marker; TAB on those rows or informational lines SHALL do nothing. Open projects SHALL have a blank row after their visible children; folded and empty projects SHALL sit together without a trailing blank row.

New lists SHALL start with populated projects open and workspaces folded except the current workspace. Refresh SHALL retain project/workspace folds by identity, including workspace folds under a folded project; newly discovered rows SHALL use the defaults. Dismissing and reopening SHALL reset folds. Folding SHALL change only the existing snapshot's presentation; new inventory and statuses SHALL appear only on explicit refresh.

A folded project SHALL append its workspace count to its primary-root location, using singular/plural, and show the most urgent state across all its workspaces using the workspace modeline priority, words and faces. Open projects SHALL leave counts and activity to their children; empty projects SHALL keep `no workspaces`.

#### Scenario: Default folds and indentation
- **GIVEN** two populated projects, several workspaces and an empty project
- **WHEN** a new list opens
- **THEN** populated projects show open markers, only the current workspace shows its buffers, other workspaces show folded markers, empty projects have no marker, and blank rows follow open projects

#### Scenario: Toggle a project and preserve children folds
- **GIVEN** a project with both open and folded workspace rows
- **WHEN** TAB folds and then unfolds the project
- **THEN** point stays on its name, folded-project count/state summarize its workspaces, and the previous workspace folds return

#### Scenario: Toggle a workspace and ignore non-foldable rows
- **GIVEN** point on a workspace with listed buffers
- **WHEN** TAB is pressed twice
- **THEN** its children hide then reappear and point remains on its name

#### Scenario: Empty project and informational rows
- **GIVEN** point on an empty project, header, blank row or buffer row
- **WHEN** TAB is pressed
- **THEN** the text and point remain as they were

#### Scenario: Refresh retains folds and dismissal resets them
- **GIVEN** an existing list whose current project and workspace have been folded
- **WHEN** workspace-list refreshes, then the list is dismissed and reopened
- **THEN** refresh retains the folds and selects the folded current project, while the new list opens that project and current workspace

#### Scenario: Folding keeps the snapshot
- **GIVEN** a list snapshot and later changes to buffer inventory or activity
- **WHEN** a row is folded then unfolded
- **THEN** the old inventory and activity remain until workspace-list is run again

#### Scenario: Navigation and activation follow visible rows
- **GIVEN** folded projects and open projects with folded workspaces
- **WHEN** the user navigates with n/p, C-n/C-p or arrows and accepts a row with RET
- **THEN** navigation and the position denominator count visible selectable rows, inventory totals include hidden rows, and RET activates the selected visible project or workspace
