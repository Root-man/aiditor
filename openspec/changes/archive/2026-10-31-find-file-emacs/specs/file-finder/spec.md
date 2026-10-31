## MODIFIED Requirements

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

## ADDED Requirements

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
