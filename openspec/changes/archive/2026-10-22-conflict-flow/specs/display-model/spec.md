# Spec Delta

## MODIFIED Requirements

### Requirement: Each window has a mode line
The display SHALL end each window with a mode line showing whether the buffer is read-only (`%%`), modified (`**`) or neither (`--`), its name followed by ` (DOS)` when its file uses CRLF line endings, ` [deleted]` when its file was deleted on disk and ` [conflict]` when its unsaved edits conflict with a change to its file, and point's line (from 1) and column (from 0).

#### Scenario: A modified file
- **WHEN** `a.txt` has been edited and point is on line 3, column 2
- **THEN** the mode line reads `** a.txt  3:2`

#### Scenario: A CRLF file
- **WHEN** an unmodified CRLF file `a.txt` is shown with point at the start
- **THEN** the mode line reads `-- a.txt (DOS)  1:0`

#### Scenario: A read-only buffer
- **WHEN** a refused binary file `a.bin` is shown
- **THEN** the mode line reads `%% a.bin  1:0`

#### Scenario: A deleted file
- **WHEN** the file of an unmodified buffer `a.txt` is deleted on disk, with point at the start
- **THEN** the mode line reads `-- a.txt [deleted]  1:0`

#### Scenario: A conflicted file
- **WHEN** `x` is typed at the start of `a.txt` and another program then rewrites the file
- **THEN** the mode line reads `** a.txt [conflict]  1:1`
