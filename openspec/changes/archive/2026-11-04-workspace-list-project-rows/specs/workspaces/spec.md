# Spec Delta

## MODIFIED Requirements

### Requirement: List the workspaces
`workspace-list` (`C-x t l`, also through `M-x`) SHALL open and select a temporary read-only `*Workspaces*` buffer in the selected window, showing every project and live workspace in a project → workspace → buffers tree. Projects SHALL be grouped by identity, in order of their first workspace's creation, with workspaces in creation order within each group. Each project heading SHALL show its name and primary root, including when names are unique. Each workspace row SHALL show its name, with the current workspace marked `*`. Beneath each workspace, the tree SHALL show its listed owned buffers, including scratch and file buffers, sorted by buffer name. Internal hidden buffers, workspace-list buffers themselves, and the global `*Messages*` SHALL be omitted from the owned-buffer branches. Projects with no workspace SHALL follow populated projects, sorted by name, and show `no workspaces`.

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

#### Scenario: Empty projects follow populated projects
- **GIVEN** projects `zeta` and `alpha` have no workspace and project `p` has one
- **WHEN** the list is opened
- **THEN** `p` appears first, followed by `alpha` and `zeta`, each showing its primary root and `no workspaces`


### Requirement: Only workspace rows activate in the workspace list
In the workspace list, `n`/`p`, `C-n`/`C-p` and the down/up arrows SHALL move selection among project and workspace rows, skipping buffer rows and stopping at the first or last selectable row. `RET` on a workspace row SHALL dismiss the invoking list and switch to that workspace using its existing window configuration. Choosing the current workspace SHALL simply dismiss the list. `RET` on a project row SHALL dismiss the list and run project-switch semantics for that project identity: show its most recently shown workspace, or create one when none exists. Buffer rows SHALL be informational: `RET` on one SHALL keep the current workspace and list visible and report `Select a project or workspace row`. Navigation from such a row SHALL select the next or previous selectable row in the requested direction, clamping to an end row when necessary. `q` or `C-g` SHALL dismiss the list and return to an ordinary buffer in the invoking workspace. Dismissal SHALL use normal buffer-kill replacement behavior. The list SHALL NOT provide individual buffer activation or destructive workspace actions.

#### Scenario: Navigate only workspaces
- **GIVEN** two adjacent workspace rows within one project, separated by their buffer children
- **WHEN** the user moves forward from the first workspace or backward from the second with `n`/`p`, `C-n`/`C-p` or arrows
- **THEN** selection moves directly to the adjacent workspace, skipping its informational buffer rows

#### Scenario: Navigate projects and workspaces
- **GIVEN** a tree with several project headings, workspace rows and buffer children
- **WHEN** the user presses `n`, `p`, `C-n`, `C-p` or the arrow keys
- **THEN** selection moves between project and workspace rows, skipping buffer rows and stopping at either end

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

#### Scenario: Project activation reuses its most recent workspace
- **GIVEN** project `p` has workspaces `a` and `b`, `a` was most recently shown, and another project is current
- **WHEN** the user selects project `p` and presses `RET`
- **THEN** `a` is shown with its saved layout and the invoking list is dismissed

#### Scenario: Project activation creates its first workspace
- **GIVEN** a listed project has no workspace
- **WHEN** the user selects that project and presses `RET`
- **THEN** a new workspace bound to that project is shown with its scratch buffer and the invoking list is dismissed


## ADDED Requirements

### Requirement: Workspace list mode line counts selectable rows
The workspace list mode line SHALL show `*Workspaces* │ N projects, M workspaces` and a one-based selectable-row position such as `2/8`. Counts SHALL describe the list snapshot, including empty projects. The position denominator SHALL count project and workspace rows only. On informational lines, position SHALL use the preceding selectable row, clamped to the first row before any selectable row.

#### Scenario: Counts include empty projects
- **GIVEN** the list snapshot contains five projects, including one empty project, and four workspaces
- **WHEN** point is on the second selectable row
- **THEN** the mode line shows `*Workspaces* │ 5 projects, 4 workspaces` and `2/9`

#### Scenario: Refresh updates totals
- **GIVEN** an existing list snapshot and a newly registered empty project
- **WHEN** the list is refreshed
- **THEN** the project count and selectable-row denominator increase by one while the workspace count stays the same
