# Spec Delta

## MODIFIED Requirements

### Requirement: List the workspaces
`workspace-list` (`C-x t l`, also through `M-x`) SHALL open and select a temporary read-only `*Workspaces*` buffer in the selected window, showing every project and live workspace in a project → workspace → buffers tree. Projects SHALL be grouped by identity, in order of their first workspace's creation, with workspaces in creation order within each group. Each project heading SHALL show its name and primary root, including when names are unique. Each workspace row SHALL show its name, with the current workspace marked `*`. Beneath each workspace, the tree SHALL show its owned file buffers, terminal buffers and registered agent-session buffers, sorted by buffer name. Scratch, agent stderr, arbitrary fileless/internal buffers, workspace-list buffers themselves and the global `*Messages*` SHALL be omitted from the owned-buffer branches. Projects with no workspace SHALL follow populated projects, sorted by name, and show `no workspaces`.

The list SHALL be an on-demand snapshot, refreshed whenever `workspace-list` is run. It SHALL reuse one list buffer per invoking workspace, with normal buffer-name disambiguation, and SHALL be read-only, fileless and unmodified. Opening or refreshing it SHALL retain all user buffer contents and the existing split structure. Initial opening SHALL place selection on the current workspace's row. Refresh through `g` or re-running the command in the selected list SHALL preserve point by stable project, workspace or buffer identity independently in each window showing that list. A missing or hidden target SHALL fall back to its visible owning workspace/project, then the current workspace or its project. Exact targets SHALL retain their character column, clamped to the new row, while fallback rows SHALL select their name. Existing scroll position SHALL remain until ordinary scrolling needs to expose point. It SHALL use the ordinary buffer display, scrolling and read-only behavior.

#### Scenario: Two workspaces
- **GIVEN** workspaces `a` on `p` and `b` on `q` exist and `b` is current
- **WHEN** `C-x t l` is typed
- **THEN** the selected list buffer shows `a` under `p` and `*b` under `q`, with the current workspace's owned buffers beneath it and the other workspace folded

#### Scenario: Project, workspace and buffer hierarchy
- **GIVEN** workspaces `a` and `b` belong to project `p`, workspace `c` belongs to project `q`, each has a scratch buffer, and `b` also owns `notes.txt`
- **WHEN** `b` runs `C-x t l`
- **THEN** the selected read-only buffer shows project `p` with children `a` and `*b`, `notes.txt` under `b`, project `q` with child `c`, and no scratch rows

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
- **THEN** each file buffer appears beneath its owner under its actual workspace, and the owned-buffer branches contain only listed workspace buffers other than the list itself

#### Scenario: Empty projects follow populated projects
- **GIVEN** projects `zeta` and `alpha` have no workspace and project `p` has one
- **WHEN** the list is opened
- **THEN** `p` appears first, followed by `alpha` and `zeta`, each showing its primary root and `no workspaces`


#### Scenario: Only user buffer kinds contribute to inventory
- **GIVEN** a workspace owns files, a terminal, registered sessions, scratch, agent stderr, arbitrary fileless buffers and Messages
- **WHEN** its list is opened or refreshed
- **THEN** only its files, terminal and registered sessions appear and contribute to buffer/unsaved counts, using type and registered identity rather than names

#### Scenario: Refresh preserves each window's target
- **GIVEN** two windows show the same list at different project, workspace or buffer rows and preceding rows are reordered or added
- **WHEN** g or workspace-list refreshes the snapshot
- **THEN** each window keeps its own target identity and column, folds remain, and new inventory is shown

#### Scenario: Removed selected buffer or workspace
- **GIVEN** a selected snapshot buffer or workspace no longer exists
- **WHEN** the list is refreshed
- **THEN** point falls back to its visible owning workspace/project, or the current workspace/project if those ancestors disappeared

### Requirement: Workspace list rows show location and state
The workspace list SHALL lay out each snapshot row in name, where and state columns, using frame 13/13b's existing named faces in either theme. Project names SHALL be bold default text and primary roots SHALL use annotation. Empty projects SHALL include `no workspaces` beside their root. Workspace names SHALL use default text, with the current mark and name in bold workspace-current. Each workspace's where column SHALL count its listed owned buffers and, when nonzero, its modified file buffers as unsaved. Its state SHALL show its most urgent agent activity, with exactly the workspace modeline's priority and status face and the session mode line's word.

Buffer names SHALL use mode-line-dim, omit their owning `<workspace>` suffix when present, and remain informational. File and terminal where columns SHALL use annotation for the directories shown by their own mode lines. Registered sessions SHALL retain the model/effort labels and item-quality faces from the same helper used by their session mode lines, including the dim separator. Buffer state SHALL use the corresponding existing status face and words: `● unsaved`, `⊘ read-only`, `[deleted]`, `[conflict]`, `◉ command`, `exit n`, `● working`, `● awaiting permission`, or `● exited` when applicable; clean/idle buffers SHALL have no state. Multiple applicable file states SHALL remain visible. Control characters in row values SHALL be rendered safely on one line. The list SHALL remain an on-demand snapshot until refreshed.

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
- **THEN** project names are bold, buffer names are dim, file/terminal locations use annotation, session labels retain item-quality faces and each state uses its existing status face

#### Scenario: Session label colours survive snapshot fitting
- **GIVEN** registered sessions have configured model and effort values with item-quality colours, including control characters in display labels
- **WHEN** their list is drawn in either theme, narrowed and widened, and the live session configuration changes without refresh
- **THEN** the retained WHERE labels use the original safe single-line text and corresponding model/effort faces, and widening restores those snapshot labels

### Requirement: Workspace list point sits on selectable names
Initial opening SHALL put point on the first character of the current workspace's name. Refresh SHALL preserve existing row identity and column with the list's ancestor fallback rule. Moving between selectable project/workspace rows SHALL put point on the first character of that row's name, after indentation and the current marker. The selectable row at point SHALL use completion-selected background independently of the current workspace marker.

#### Scenario: Point follows names
- **GIVEN** a current workspace and another project/workspace are listed
- **WHEN** the list opens and the user moves with any workspace-list navigation binding
- **THEN** point starts on the selected name and that row has completion-selected background while the current workspace keeps its workspace-current face

### Requirement: Workspace list folds preserve snapshot presentation
TAB SHALL fold or unfold the project or workspace row at point, retaining point on that row's name. A row with children SHALL show `▾` when open and `▸` when folded in session-thought, with indentation rather than tree connectors. Rows without children SHALL have no fold marker; TAB on those rows or informational lines SHALL do nothing. Exactly one separator row with the existing separator-face horizontal-rule decoration SHALL separate every adjacent pair of projects, including folded and empty projects. There SHALL be no leading or trailing separator; navigation SHALL skip separators.

New lists SHALL start with populated projects open and workspaces folded except the current workspace. Refresh SHALL retain project/workspace folds by identity, including workspace folds under a folded project; newly discovered rows SHALL use the defaults. Dismissing and reopening SHALL reset folds. Folding SHALL change only the existing snapshot's presentation; new inventory and statuses SHALL appear only on explicit refresh.

A folded project SHALL append its workspace count to its primary-root location, using singular/plural, and show the most urgent state across all its workspaces using the workspace modeline priority, words and faces. Open projects SHALL leave counts and activity to their children; empty projects SHALL keep `no workspaces`.

#### Scenario: Default folds and indentation
- **GIVEN** two populated projects, several workspaces and an empty project
- **WHEN** a new list opens
- **THEN** populated projects show open markers, only the current workspace shows its buffers, other workspaces show folded markers, empty projects have no marker, and horizontal rules separate adjacent projects

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
- **THEN** refresh retains the folds and the selected row by identity or visible ancestor, while the new list opens that project and current workspace

#### Scenario: Folding keeps the snapshot
- **GIVEN** a list snapshot and later changes to buffer inventory or activity
- **WHEN** a row is folded then unfolded
- **THEN** the old inventory and activity remain until workspace-list is run again

#### Scenario: Navigation and activation follow visible rows
- **GIVEN** folded projects and open projects with folded workspaces
- **WHEN** the user navigates with n/p, C-n/C-p or arrows and accepts a row with RET
- **THEN** navigation and the position denominator count visible selectable rows, inventory totals include hidden rows, and RET activates the selected visible project or workspace

#### Scenario: Rules separate every project
- **GIVEN** open, folded and empty projects
- **WHEN** folds change, the snapshot refreshes or the window resizes
- **THEN** exactly one decorated horizontal-rule row remains between each adjacent pair of projects with no leading/trailing rule, and project/workspace navigation skips it

### Requirement: Workspace list keys live in the echo line
The workspace list SHALL start with its first project row, without a header or leading blank row. While the selected window shows a workspace list and no message or minibuffer is up, the echo line SHALL show `n/p move   RET switch   TAB fold   g refresh   q quit`. Keys SHALL use bold text in the `workspace-current` foreground and action labels SHALL use `mode-line-dim`, as frames 13–13d and 6d. Selecting another window SHALL remove these hints even if the list remains visible elsewhere. A message SHALL replace the hints until the next key; timers SHALL retain the message. An open minibuffer SHALL retain its ordinary prompt. Hints SHALL wrap between complete actions into a second row when they do not fit, and the windows SHALL yield the extra echo height.

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
