# Spec Delta

## ADDED Requirements

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
Opening or refreshing the list SHALL put point on the first character of the current workspace's name. Moving between selectable project/workspace rows SHALL put point on the first character of that row's name, after indentation and the current marker. The selectable row at point SHALL use completion-selected background independently of the current workspace marker.

#### Scenario: Point follows names
- **GIVEN** a current workspace and another project/workspace are listed
- **WHEN** the list opens and the user moves with any workspace-list navigation binding
- **THEN** point starts on the selected name and that row has completion-selected background while the current workspace keeps its workspace-current face
