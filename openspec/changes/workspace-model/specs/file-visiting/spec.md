# Spec Delta

## MODIFIED Requirements

### Requirement: Visiting a file already visited switches to its buffer
When a buffer of the current workspace already visits the named path, `find-file` SHALL make that buffer current without reading the file and without making another buffer. A buffer visiting the path in another workspace SHALL NOT be reused: the file SHALL be read into a new buffer of the current workspace.

#### Scenario: Same file twice
- **WHEN** `d/a.txt` is visited, edited, and `C-x C-f` names `d/a.txt` again
- **THEN** there is one buffer visiting it, it is current and keeps its edits

#### Scenario: Same file in another workspace
- **WHEN** `d/a.txt` is visited, a new workspace is made, and `d/a.txt` is visited again
- **THEN** there are two buffers visiting it, one per workspace, and the new workspace's is current
