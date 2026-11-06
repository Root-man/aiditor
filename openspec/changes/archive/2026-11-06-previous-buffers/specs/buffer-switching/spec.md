# Spec Delta

## ADDED Requirements

### Requirement: Cycling a window's buffers
Each window SHALL remember the buffers it showed before the current one, most recent first, and the buffers `previous-buffer` left. Showing another buffer in a window by any command other than these two SHALL make the buffer it left the most recent previous one and forget the buffers `previous-buffer` left. A window made by splitting SHALL start with the history of the window it was split from.

`previous-buffer` (`C-x <left>`) SHALL show the selected window's most recent previous buffer and remember the buffer it left for `next-buffer`. `next-buffer` (`C-x <right>`) SHALL show the buffer `previous-buffer` left most recently and make the buffer it left the most recent previous one. Neither SHALL change the order `C-x b` lists buffers in. A window with no previous buffer SHALL instead show the most recent listed buffer of the current workspace, or `*Messages*`, that it isn't showing and that `next-buffer` would not show; a window with nothing for `next-buffer` SHALL instead show the least recent such buffer that `previous-buffer` would not show. With no such buffer they SHALL fail with `No previous buffer` or `No next buffer`. Both SHALL fail in the minibuffer with `Cannot switch buffers in minibuffer window`.

#### Scenario: Back and forward
- **WHEN** a window shows `a`, then `b`, then `c`, and `C-x <left>` is typed twice, then `C-x <right>`
- **THEN** the window shows `b`, then `a`, then `b`

#### Scenario: Two windows keep their own histories
- **WHEN** one window shows `a` then `b`, the other window shows `c` then `d`, and `C-x <left>` is typed in each
- **THEN** the first window shows `a` and the second shows `c`

#### Scenario: A window with no history
- **WHEN** a new workspace's window has only shown its `*scratch*`, file `a` was made in another window, and `C-x <left>` is typed
- **THEN** the window shows the workspace's most recent other listed buffer

#### Scenario: Showing a buffer forgets the way forward
- **WHEN** a window shows `a`, then `b`, `C-x <left>` is typed, then `c` is shown with `C-x b`, and `C-x <right>` is typed
- **THEN** the window does not show `b`

## MODIFIED Requirements

### Requirement: Killing a buffer
`kill-buffer` (`C-x k`) SHALL read a buffer name, completing over the listed buffers of the current workspace and `*Messages*` with the current one first, and kill that buffer. The buffer SHALL drop out of every window's history. Every window showing it SHALL show its own most recent previous buffer of the current workspace, or `*Messages*`, instead, or, with none, the most recent other listed buffer of the current workspace, or `*Messages*`. A buffer of another workspace SHALL NOT replace it. A name no such buffer has, another workspace's buffer included, SHALL fail with `No such buffer <name>`. `*Messages*` and each workspace's `*scratch*` SHALL NOT be killed: the kill SHALL fail with `Can't kill <name>`.

#### Scenario: Kill the current buffer
- **WHEN** a file is visited from `*scratch*` and `C-x k RET` is typed
- **THEN** the file's buffer is gone and `*scratch*` is current

#### Scenario: Each window goes back to its own buffer
- **WHEN** one window shows `a` then `c`, the other shows `b` then `c`, `b` is the most recent buffer besides `c`, and `c` is killed
- **THEN** the first window shows `a` and the second shows `b`

#### Scenario: A killed buffer leaves every history
- **WHEN** a window shows `a`, then `b`, then `c`, `b` is killed, and `C-x <left>` is typed
- **THEN** the window shows `a`

#### Scenario: Scratch and Messages can't be killed
- **WHEN** `C-x k RET` is typed in `*scratch*`, or `C-x k *Messages* RET`
- **THEN** the echo area shows `Can't kill *scratch*` or `Can't kill *Messages*` and the buffer stays

#### Scenario: Unknown name
- **WHEN** `C-x k nope RET` is typed
- **THEN** the echo area shows `No such buffer nope` and no buffer is killed

#### Scenario: Another workspace's buffer is not killed
- **WHEN** file `a` is visited in one workspace, a new workspace is made, and `C-x k a RET` is typed
- **THEN** the echo area shows `No such buffer a` and `a` still exists

#### Scenario: Another workspace's buffer is not shown
- **WHEN** a file is visited in one workspace, a new workspace is made and shows it, then a file visited in the new workspace is killed
- **THEN** the window shows a buffer of the new workspace, not the other workspace's file
