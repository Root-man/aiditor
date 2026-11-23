# diff-revisions Specification

## Purpose
Compares two revisions of one root, or a revision and the working tree, in the diff view, as `git diff A B` does.

## Requirements

### Requirement: Asking for a root and two revisions
`diff-revisions` SHALL ask for a root only when the current project has several git roots, then `Old revision: ` and `New revision (empty for working tree): `, each completing over that root's branches and tags and taking typed text git can resolve. An empty new revision SHALL mean the working tree. A project with no git root SHALL echo `No git root`.

#### Scenario: One git root
- **GIVEN** a project whose only git root has branches `main` and `topic` and a tag `v1`
- **WHEN** `diff-revisions` runs
- **THEN** it asks `Old revision: ` at once, offering `main`, `topic` and `v1`

#### Scenario: Several git roots
- **GIVEN** a project with two git roots and one root outside git
- **WHEN** `diff-revisions` runs
- **THEN** it asks for a root, offering the two git roots only

#### Scenario: An empty old revision
- **WHEN** the old revision is answered empty
- **THEN** it echoes `No revision` and nothing opens

### Requirement: The view lists the files git diff lists
Once git answers, the diff view SHALL open with the root's heading naming `<old> → <new>` (`working tree` for an empty new revision) and, straight under it, each file `git diff --name-status` lists between them, with its status letter. A revision git can't resolve SHALL echo git's error and open nothing. Running `diff-revisions` or `diff-status` while the other's view is open SHALL replace it.

#### Scenario: Two revisions
- **GIVEN** branches `main` and `topic` that differ in a modified, an added and a deleted file
- **WHEN** `diff-revisions` is answered `main` and `topic`
- **THEN** the list shows the root with `main → topic` and the three files with `M`, `A` and `D`

#### Scenario: An unknown revision
- **WHEN** the old revision is `nope` and git can't resolve it
- **THEN** git's error is echoed and the layout is unchanged

### Requirement: The new side is editable only as the working tree
Against the working tree the diff window SHALL show the file's own buffer, which edits as in a status diff; between two revisions it SHALL show a read-only buffer named `<path> (<new>)` holding that revision's text. The mode line's sides SHALL read `<old> → <new>` or `<old> → working tree`.

#### Scenario: Against the working tree
- **WHEN** a file of a `HEAD` → working tree diff is shown and a character is typed in the diff window
- **THEN** the file's buffer holds it and the mode line reads `HEAD → working tree`

#### Scenario: Between two revisions
- **WHEN** a file of a `main` → `topic` diff is shown
- **THEN** the diff window shows read-only `<path> (topic)` with `topic`'s text and the mode line reads `main → topic`

### Requirement: Nothing is staged or discarded
Stage, unstage and discard SHALL echo `This diff can't be changed` in either window and change nothing.

#### Scenario: Staging from the list
- **WHEN** `s` is pressed on a file of a revisions list
- **THEN** `This diff can't be changed` is echoed and no git command runs

### Requirement: Refresh resolves the revisions again
`diff-refresh` SHALL list the files between the same revision names again, so a branch that moved shows its new files; a failure SHALL show git's error on the root's heading.

#### Scenario: A branch moved
- **GIVEN** a `main` → `topic` list, and `topic` then gains a commit touching another file
- **WHEN** `g` is pressed
- **THEN** the list asks git again with `main` and `topic` and shows the new file
