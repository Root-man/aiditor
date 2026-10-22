# Spec Delta

## MODIFIED Requirements

### Requirement: Clean buffers revert to a changed file
When a check finds a buffer's file present with a hash other than the buffer's disk hash, an unmodified buffer SHALL be reverted from the file as a revert does: point, mark and markers stay with their text, the old text is one undo group, and the buffer ends unmodified with the file's hash. This SHALL hold for every buffer visiting the file, in every workspace, and for a clean buffer in the deleted state or visiting a file that did not exist when visited. A buffer whose disk hash equals the file's SHALL be left alone, so a buffer's own save does not revert it. A modified buffer SHALL keep its text, its modified flag and its disk hash, and become conflicted (see file-conflicts). An auto-revert SHALL NOT echo; a failed one SHALL echo its error as `revert-buffer` does.

#### Scenario: A save in another workspace
- **WHEN** buffers in two workspaces visit the same file, one of them is edited and saved, and the watcher reports the file
- **THEN** the other buffer holds the saved text and is unmodified, and the saving buffer is not reverted

#### Scenario: Another program writes the file
- **WHEN** another program rewrites the file of an unmodified buffer whose point is on a line the change leaves alone, and the watcher reports it
- **THEN** the buffer holds the new text, is unmodified, point is on the same line, `C-/` brings the old text back, and the echo area is empty

#### Scenario: A modified buffer is not reverted
- **WHEN** another program rewrites the file of a modified buffer and the watcher reports it
- **THEN** the buffer keeps its text, stays modified and is conflicted, and `C-x C-s` asks how to resolve the conflict instead of saving

#### Scenario: A deleted file comes back with other content
- **WHEN** an unmodified buffer's file is deleted, reported, then created again with other content and reported
- **THEN** the buffer holds the new content, is not in the deleted state, and an edit followed by `C-x C-s` writes the file
