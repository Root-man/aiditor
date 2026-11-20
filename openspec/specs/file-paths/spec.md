# file-paths Specification

## Purpose
Commands that hand the current buffer's file path to the user, relative to its git repository root or absolute, for pasting into prompts, terminals and other apps.

## Requirements

### Requirement: copy-relative-path copies the path from the file's git repository root
`copy-relative-path` SHALL copy the current buffer's file path relative to the root of the git repository holding the file: the nearest directory above the file that contains `.git`, a directory or a file (a linked worktree or submodule). Project roots SHALL NOT affect it. Outside a git repository it SHALL fail with `File is not in a git repository` and copy nothing.

#### Scenario: A file in a repository
- **WHEN** `/repo/.git` exists and `copy-relative-path` runs in a buffer visiting `/repo/src/a.rs`
- **THEN** `src/a.rs` is copied

#### Scenario: The repository is not the project root
- **WHEN** the project's primary root is `/work`, `/work/lib/.git` exists and `copy-relative-path` runs in a buffer visiting `/work/lib/src/a.rs`
- **THEN** `src/a.rs` is copied

#### Scenario: A linked worktree or submodule
- **WHEN** `/repo/.git` exists, `/repo/sub/.git` is a file and `copy-relative-path` runs in a buffer visiting `/repo/sub/b.rs`
- **THEN** `b.rs` is copied

#### Scenario: A file outside every repository
- **WHEN** no directory above `/tmp/x.txt` contains `.git` and `copy-relative-path` runs in a buffer visiting it
- **THEN** the echo area shows `File is not in a git repository` and nothing is copied

### Requirement: copy-absolute-path copies the absolute path
`copy-absolute-path` SHALL copy the current buffer's absolute file path, whether or not a git repository holds it.

#### Scenario: A file in a repository
- **WHEN** `copy-absolute-path` runs in a buffer visiting `/repo/src/a.rs`
- **THEN** `/repo/src/a.rs` is copied

#### Scenario: A file outside every repository
- **WHEN** `copy-absolute-path` runs in a buffer visiting `/tmp/x.txt`
- **THEN** `/tmp/x.txt` is copied

### Requirement: A copied path goes to the kill ring and the clipboard
A path copied by `copy-relative-path` or `copy-absolute-path` SHALL become a new entry at the front of the kill ring and SHALL be written to the system clipboard, and the echo area SHALL show `Copied <path>`.

#### Scenario: The path yanks back
- **WHEN** `copy-absolute-path` runs in a buffer visiting `/repo/a.rs`, then `C-y` is pressed in another buffer
- **THEN** `/repo/a.rs` is inserted, the clipboard holds `/repo/a.rs` and the echo area showed `Copied /repo/a.rs`

### Requirement: Copying a path needs a file buffer
`copy-relative-path` and `copy-absolute-path` SHALL fail with `Buffer is not visiting a file` and copy nothing when the current buffer visits no file.

#### Scenario: Run in *scratch*
- **WHEN** `copy-relative-path` or `copy-absolute-path` runs in `*scratch*`
- **THEN** the echo area shows `Buffer is not visiting a file` and the kill ring and clipboard are unchanged
