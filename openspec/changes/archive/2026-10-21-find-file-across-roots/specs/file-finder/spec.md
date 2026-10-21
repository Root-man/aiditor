# Spec Delta

## Purpose

Finds any file in any root of the current workspace's project by fuzzy matching its path, so files in additional roots are as close as those in the primary repo.

## ADDED Requirements

### Requirement: Find a file in the project
`project-find-file` SHALL prompt `Find file in <project>: `, `<project>` being the current workspace's project, and complete with fuzzy matching over every file in that project's index as `<root name>/<path relative to the root>`, the primary root's files first, each root's sorted. `RET` SHALL visit the selected file, as `find-file` visits it.

#### Scenario: Files from two roots
- **WHEN** the project's primary `/r/app` holds `src/main.rs` and its root `/r/lib` holds `lib.rs`, and `project-find-file` is called
- **THEN** the list offers `app/src/main.rs` and `lib/lib.rs`

#### Scenario: Visit a file of an additional root
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
Text that matches no candidate, or is returned with `M-RET`, SHALL be visited under the root whose name is its first component, the rest of the text taken from that root, or else under the primary root, so that a file not yet on disk can be made.

#### Scenario: New file in a root
- **WHEN** `lib/new.rs` is typed in a project with a root named `lib` and no such file, and `M-RET` pressed
- **THEN** a buffer visiting `/r/lib/new.rs` opens, empty

#### Scenario: No root named
- **WHEN** `notes.md` is typed, matching nothing, and `RET` pressed
- **THEN** a buffer visiting `<primary>/notes.md` opens

### Requirement: The finder previews the selected file
`project-find-file` SHALL preview the selected candidate's file in the window it was opened from, as a previewing prompt does.

#### Scenario: Moving the selection
- **WHEN** `C-n` selects `b/x` in `project-find-file`
- **THEN** the window the finder was opened from shows `/r/b/x`'s text, and the minibuffer stays selected
