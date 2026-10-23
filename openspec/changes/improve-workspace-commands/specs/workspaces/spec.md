# Spec Delta

## MODIFIED Requirements

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

## ADDED Requirements

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
- **GIVEN** ordinary point movement or a click has placed point on a project or buffer row
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
