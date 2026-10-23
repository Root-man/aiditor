# Spec Delta

## MODIFIED Requirements

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

## ADDED Requirements

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
