## ADDED Requirements

### Requirement: vterm opens a terminal buffer
`vterm` (through `M-x`) SHALL spawn the user's login shell in a new terminal buffer of the current workspace and show it in the selected window. The buffer SHALL be named `vterm<n><workspace>`, `n` the smallest number from 1 no buffer of that name has. The shell's working directory SHALL be the current buffer's file's directory, or the primary root of the current workspace's project when the buffer visits no file.

#### Scenario: Names count within a workspace
- **WHEN** `M-x vterm` runs twice in workspace `work`, then once in workspace `fix`
- **THEN** the buffers are `vterm1<work>`, `vterm2<work>` and `vterm1<fix>`

#### Scenario: A freed number is reused
- **WHEN** `vterm1<work>` and `vterm2<work>` exist, `vterm1<work>` is killed and `M-x vterm` runs in `work`
- **THEN** the new buffer is `vterm1<work>`

#### Scenario: Working directory from the file
- **WHEN** `M-x vterm` runs from a buffer visiting `/r/app/src/main.rs`
- **THEN** the shell is spawned in `/r/app/src`

#### Scenario: Working directory from the project
- **WHEN** `M-x vterm` runs from `*scratch*` in a workspace whose project's primary root is `/r/app`
- **THEN** the shell is spawned in `/r/app`

### Requirement: The terminal is sized to its smallest window
A live terminal's grid and PTY SHALL be as many rows as the text area (the window less its mode line) and as many columns as fit in the narrowest and shortest window of the current workspace showing it, taking rows and columns independently, as Emacs's `window-adjust-process-window-size-smallest`. When that size changes, the core SHALL resize the grid and send one `TerminalOp::Resize`. A terminal no window shows, or one before the frame size is known, SHALL keep its size; a new one starts at 24x80 until the frame size is known.

#### Scenario: Spawned at the window's size
- **WHEN** the frame is 80 cells by 25 lines and `M-x vterm` runs in one window
- **THEN** the shell is spawned with 24 rows and 80 columns

#### Scenario: Splitting shrinks it
- **WHEN** a terminal is shown in the only window and `C-x 2` splits it
- **THEN** the core resizes the PTY to the smaller of the two windows' text areas

#### Scenario: Resizing the frame
- **WHEN** the frame showing a terminal gets smaller
- **THEN** the core resizes the PTY to the new size, once

### Requirement: The buffer follows the shell
When the shell exits, its buffer SHALL be killed. When the spawn fails, the buffer SHALL be killed and the reason shown in the echo area. When a terminal buffer is killed by any path, its shell SHALL be killed.

#### Scenario: exit
- **WHEN** the terminal service reports a terminal's shell exited
- **THEN** its buffer is killed and nothing is sent to that terminal

#### Scenario: Spawn failure
- **WHEN** the terminal service reports a spawn failed with `/x: not a directory`
- **THEN** the buffer is killed and the echo area shows `/x: not a directory`

#### Scenario: Killing a workspace
- **WHEN** a workspace holding two terminals is killed
- **THEN** the core sends `TerminalOp::Kill` for both

### Requirement: Killing a live terminal asks first
`kill-buffer` on a terminal whose shell has not exited SHALL ask `Buffer <name> has a running process; kill it? `, completing over `no` then `yes`. Only `yes` SHALL kill the buffer and its shell.

#### Scenario: Yes
- **WHEN** `kill-buffer` names a live terminal and the answer is `yes`
- **THEN** the buffer is killed and `TerminalOp::Kill` is sent for it

#### Scenario: No
- **WHEN** the answer is `no`
- **THEN** the buffer stays and nothing is sent to the terminal
