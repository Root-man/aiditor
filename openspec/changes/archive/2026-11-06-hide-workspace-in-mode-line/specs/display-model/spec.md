# Spec Delta

## MODIFIED Requirements

### Requirement: Each window has a mode line
The display SHALL end each window with a mode line of two groups of segments, a segment appearing only when it applies. For a terminal buffer, the left group SHALL hold its name, then the shell's directory, and the right group the running command, the last command's non-zero exit status, the input mode, then point's line and column in copy mode. For any other buffer not visiting a file, the left group SHALL hold whether it is read-only (`%%`), modified (`**`) or neither (`--`) with its name, then any agent mode, and the right group point's line (from 1) and column (from 0). The name a terminal's or fileless buffer's mode line shows SHALL leave out a `<workspace>` suffix ending the buffer's name when it names the current workspace, since the workspace modeline already shows it; the buffer's name itself SHALL keep the suffix. For a buffer visiting a file, the left group SHALL hold `●` when modified and `⊘` when read-only, the file's directory relative to its project root, after that root's name in a project of several roots, and its name, then `[deleted]` when its file was deleted on disk or `[conflict]` when its unsaved edits conflict with a change to its file, then its major mode. The right group SHALL hold the active region's size, `UTF-8 CRLF` when its file uses CRLF line endings, its branch, point's line and column, then its position.

#### Scenario: A modified file
- **WHEN** `src/a.rs` under a project root has been edited and point is on line 3, column 2
- **THEN** the left group's segments are `● src/a.rs` and `Rust`, and the right group's `3:2` and the position

#### Scenario: A clean file at its root
- **WHEN** an unmodified `a.txt` at a project root is shown
- **THEN** the left group's segments are `a.txt` and `Fundamental`

#### Scenario: A file in a project of several roots
- **WHEN** `src/a.rs` under a root named `api` is shown in a project with two roots
- **THEN** the left group's first segment is `api/src/a.rs`

#### Scenario: A file outside the project's roots
- **WHEN** `/etc/hosts` is shown in a project with no root above it
- **THEN** the left group starts `/etc/hosts`

#### Scenario: A CRLF file
- **WHEN** an unmodified CRLF file `a.txt` that fits its window is shown with point at the start
- **THEN** the right group's segments are `UTF-8 CRLF`, `1:0` and `All`

#### Scenario: A read-only buffer
- **WHEN** a refused binary file `a.bin` is shown
- **THEN** the left group starts `⊘ a.bin`

#### Scenario: Read-only and modified
- **WHEN** a recovered read-only file is shown modified
- **THEN** the left group starts `● ⊘`

#### Scenario: A deleted file
- **WHEN** the file of an unmodified buffer `a.txt` is deleted on disk, with point at the start
- **THEN** the left group's segments are `a.txt`, `[deleted]` and `Fundamental`

#### Scenario: A conflicted file
- **WHEN** `x` is typed at the start of `a.txt` and another program then rewrites the file
- **THEN** the left group's segments are `● a.txt`, `[conflict]` and `Fundamental`

#### Scenario: A file on a branch
- **WHEN** a file in a repository on `main` is shown
- **THEN** the right group has `⎇ main` before `line:col`

#### Scenario: A file with a diff
- **WHEN** a file git reports as changed, on `main`, is shown
- **THEN** that segment reads `⎇ main ±`

#### Scenario: A buffer with no file
- **WHEN** `*scratch*` is shown modified with point on line 3, column 2
- **THEN** the left group is `** *scratch*` and the right group `3:2`

#### Scenario: A terminal
- **WHEN** terminal `t`'s shell is in `/srv` and a command is running
- **THEN** the left group's segments are `t` and `/srv`, and the right group's `◉ <command>` and `char`

#### Scenario: A workspace's scratch
- **WHEN** `*scratch*<work>` is shown unmodified in workspace `work`
- **THEN** the left group is `-- *scratch*`, and the buffer is still named `*scratch*<work>`

#### Scenario: A workspace's terminal
- **WHEN** `vterm1<work>` is shown in workspace `work` with its shell in `/srv`
- **THEN** the left group's segments are `vterm1` and `/srv`

#### Scenario: An agent's stderr
- **WHEN** `*claude stderr*<work>` is shown in workspace `work`
- **THEN** the left group is `%% *claude stderr*`

#### Scenario: A suffix that doesn't end the name
- **WHEN** a second stderr buffer `*claude stderr*<work><2>` is shown in workspace `work`
- **THEN** the left group is `%% *claude stderr*<work><2>`

#### Scenario: A buffer of no workspace
- **WHEN** `*Messages*` is shown
- **THEN** the left group is `%% *Messages*`
