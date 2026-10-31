# file-finder Specification

## Purpose
Finds any file in any root of the current workspace's project by fuzzy matching its path, so files in additional roots are as close as those in the primary repo.

## Requirements

### Requirement: Find a file in the project
`project-find-file` SHALL prompt `Find file in <project>: `, `<project>` being the current workspace's project, and complete with fuzzy matching over every file in that project's index as `<root name>/<path relative to the root>`, the primary root's files first, each root's sorted. `RET` SHALL visit the selected file, as `find-file` visits it; with no selection it SHALL visit the typed text through the captured root mapping.

#### Scenario: Files from two roots
- **GIVEN** the project's primary `/r/app` holds `src/main.rs` and its root `/r/lib` holds `lib.rs`
- **WHEN** `project-find-file` is called
- **THEN** the list offers `app/src/main.rs` and `lib/lib.rs`

#### Scenario: Visit a file of an additional root
- **GIVEN** that project finder is open
- **WHEN** `lib.rs` is typed and `RET` pressed
- **THEN** `/r/lib/lib.rs` is visited in the selected window

### Requirement: Root names
A root's name SHALL be the last component of its path. A name an earlier root of the project already has, the primary counting first, SHALL get `<2>`, then `<3>`, and so on.

#### Scenario: Two roots with one name
- **WHEN** a project has the primary `/a/lib` and a root `/b/lib`, each holding `x.rs`
- **THEN** the list offers `lib/x.rs` and `lib<2>/x.rs`, and each visits its own root's file

### Requirement: Index on first use
When the project has no index yet, `project-find-file` SHALL start indexing it, as `project-index` does, and open its prompt at once. When that walk answers while the prompt is still open, its files SHALL join the list. A project indexed before SHALL be offered its kept files without walking again.

#### Scenario: First use
- **WHEN** `project-find-file` is called on a project never indexed, and the walk answers before `RET`
- **THEN** the echo area says `Indexing <project>...` and the list then offers the walk's files

#### Scenario: Later use
- **WHEN** `project-find-file` is called on a project indexed before
- **THEN** its files are offered at once and no walk starts

### Requirement: Visit a typed name
Text that matches no candidate, or is returned with `M-RET`, SHALL be visited under the root whose name is its first component, the rest of the text taken from that root, or else under the primary root, so that a file not yet on disk can be made. This resolution SHALL work before the initial index completes, using the root names captured for that finder session. Duplicate root names SHALL use the same `<2>`, `<3>` suffixes as indexed candidates.

#### Scenario: New file in a root
- **GIVEN** a project has a root `/r/lib` named `lib` and no `new.rs` there
- **WHEN** `lib/new.rs` is typed and `M-RET` pressed
- **THEN** a buffer visiting `/r/lib/new.rs` opens, empty

#### Scenario: No root named
- **GIVEN** a project finder is open
- **WHEN** `notes.md` is typed, matching nothing, and `RET` pressed
- **THEN** a buffer visiting `<primary>/notes.md` opens

#### Scenario: Before the initial index answers
- **GIVEN** a project's roots are `/a/lib` and `/b/lib` and its first walk is pending
- **WHEN** `lib<2>/new.rs` is typed and `M-RET` pressed
- **THEN** an empty buffer visits `/b/lib/new.rs` without waiting for the index

### Requirement: The finder previews the selected file
`project-find-file` SHALL preview the selected candidate's file in the window it was opened from, as a previewing prompt does.

#### Scenario: Moving the selection
- **WHEN** `C-n` selects `b/x` in `project-find-file`
- **THEN** the window the finder was opened from shows `/r/b/x`'s text, and the minibuffer stays selected

### Requirement: A finder keeps its root mapping
An open project finder SHALL keep the root-name-to-path mapping associated with its displayed candidates, or the current project's roots at opening when no initial index result exists. Previewing, selecting and typing a name SHALL use that mapping for the life of the prompt. A reloaded or reindexed project SHALL NOT reinterpret existing candidates against different roots. Results for a different root mapping SHALL NOT be appended into that prompt. A newly opened finder SHALL use the latest available project/index snapshot according to the existing indexing rules.

#### Scenario: Roots change while candidates are displayed
- **GIVEN** a finder displays `lib/x.rs` for `/old/lib/x.rs`
- **WHEN** reload changes the project's root to `/new/lib` and `lib/x.rs` is accepted from the still-open finder
- **THEN** the accepted path is `/old/lib/x.rs`

#### Scenario: Initial indexing is superseded
- **GIVEN** a finder opened with no index and captured its project roots
- **WHEN** a reload changes those roots before an index result arrives
- **THEN** the prompt retains its captured mapping and only a compatible result can supply its candidates

### Requirement: Project file completion starts without a selection
An empty project-find-file query SHALL show no selected candidate and a padded `*/N` count, including before and after first-use indexing. C-n from no selection SHALL select the first match; C-p SHALL select the last. TAB without a selection SHALL complete the first match. A nonempty query SHALL select its first match as before, and clearing the query SHALL clear the selection. An unselected finder SHALL leave its original buffer displayed.

#### Scenario: Opening and navigating
- **GIVEN** a project index holds five files
- **WHEN** project-find-file opens and then C-n is pressed
- **THEN** the count changes from `*/5` to `1/5` and the first file is previewed only after selection

#### Scenario: First-use indexing
- **GIVEN** project-find-file is waiting for its first index
- **WHEN** the index supplies files while the query is empty
- **THEN** the list fills while retaining no selection and the original buffer

#### Scenario: Completing and clearing
- **GIVEN** an unselected project file list
- **WHEN** TAB completes the first match and the query is subsequently cleared
- **THEN** completion uses that first file and the cleared query returns to no selection

### Requirement: Project file completion annotates visible paths
Project-find-file SHALL show the same Unix mode, decimal size and modification age columns as find-file, outside fuzzy matching. Metadata reads SHALL run without blocking the editor and be limited to visible matches rather than every indexed file. Requested annotations SHALL be reused for the life of that prompt session; missing metadata SHALL show `—` for unavailable fields. Files SHALL resolve through that finder's captured root mapping. Replies from a closed or superseded prompt session SHALL be ignored. The project index SHALL retain its existing path-only contents and ordering.

#### Scenario: A large index
- **GIVEN** a project index has more files than its ten visible rows
- **WHEN** project-find-file opens or scrolls
- **THEN** metadata is requested only for newly visible files, and revisiting those rows reuses their annotations

#### Scenario: Multiple roots and a reload
- **GIVEN** the displayed candidates use captured root labels, including a duplicate root name
- **WHEN** project settings reload and annotations arrive
- **THEN** each annotation still describes its displayed candidate's original path

#### Scenario: A reopened finder
- **GIVEN** metadata is being read for a project finder that closes
- **WHEN** its reply arrives after another finder opens
- **THEN** the newer finder retains its own annotations

#### Scenario: Names alone are matched
- **GIVEN** a visible file's metadata contains `41 B` but its name does not match `41`
- **WHEN** `41` is typed
- **THEN** that metadata does not cause the file to match
