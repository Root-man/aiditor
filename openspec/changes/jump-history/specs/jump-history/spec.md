# Spec Delta

## Purpose

Jump history: recording the position a jump leaves, so `M-,` and `C-M-,` move back and forward across buffers, as Emacs 29's xref history does.

## ADDED Requirements

### Requirement: Jumps record the position they leave
Before a jump moves the selected window, the window's buffer and point SHALL be recorded in the current workspace's back list. Jumps are:
- showing another buffer in the selected window, by `switch-to-buffer`, `switch-to-buffer-other-window`, `find-file`, `project-find-file` or `project-grep`;
- `project-grep` going to a hit in the current buffer;
- `swiper` moving point.

A recorded position SHALL follow edits to its buffer.

#### Scenario: Buffer switch
- **WHEN** at point 3 in `a.txt`, `C-x b b.txt RET` is typed
- **THEN** the back list's newest position is `a.txt` at 3

#### Scenario: Grep hit in another file
- **WHEN** at point 2 in `a.txt`, a `project-grep` hit in `b.txt` is chosen
- **THEN** `b.txt` shows at the hit and the back list's newest position is `a.txt` at 2

#### Scenario: Grep hit in the current buffer
- **WHEN** at point 0 in `a.txt`, a `project-grep` hit on line 3 of `a.txt` is chosen
- **THEN** the back list's newest position is `a.txt` at 0

#### Scenario: Swiper
- **WHEN** swiper moves point from 0 to a later line
- **THEN** the back list's newest position is at 0

#### Scenario: Positions follow edits
- **WHEN** `a.txt` at 3 is recorded, and 2 chars are then inserted at the start of `a.txt`
- **THEN** the recorded position is `a.txt` at 5

### Requirement: Only jumps record
Moving within a buffer by any other command, previewing in the minibuffer, `C-g` out of a prompt, and switching to the buffer already shown SHALL record nothing. A jump from the newest back position SHALL add nothing to the back list.

#### Scenario: Preview and quit
- **WHEN** swiper previews several lines and is quit with `C-g`
- **THEN** the back list is unchanged

#### Scenario: Same buffer
- **WHEN** `C-x b` names the buffer already shown
- **THEN** the back list is unchanged

### Requirement: Each workspace has its own history
Every window of a workspace SHALL share that workspace's back and forward lists, and a workspace SHALL NOT see another's. The back list SHALL keep at most 100 positions, dropping the oldest. Every jump SHALL empty the forward list. Killing a workspace SHALL drop its history.

#### Scenario: Separate workspaces
- **WHEN** a jump is made in workspace `one`, then workspace `two` is made current and `M-,` is typed
- **THEN** `two`'s history is empty and `At start of xref history` shows

#### Scenario: A new jump clears forward
- **WHEN** after a jump and `M-,`, another jump is made and `C-M-,` is typed
- **THEN** `At end of xref history` shows

### Requirement: M-, goes back
`M-,` (`xref-go-back`) SHALL take positions off the back list, newest first, skipping those whose buffer was killed and those equal to the selected window's buffer and point. It SHALL put the selected window's position on the forward list, then show the position's buffer in the selected window with point at it, centering its line when out of view. That buffer SHALL become the most recent. With nothing left it SHALL fail with `At start of xref history`.

#### Scenario: Back after a grep jump
- **WHEN** at point 2 in `a.txt`, a `project-grep` hit in `b.txt` is chosen and `M-,` is typed
- **THEN** `a.txt` shows with point at 2

#### Scenario: Back across several jumps
- **WHEN** from `a.txt` the user switches to `b.txt`, then to `c.txt`, then types `M-, M-,`
- **THEN** `a.txt` shows, and `C-M-, C-M-,` shows `c.txt` again

#### Scenario: A killed buffer is skipped
- **WHEN** from `a.txt` the user switches to `b.txt`, then to `c.txt`, kills `b.txt` and types `M-,`
- **THEN** `a.txt` shows

### Requirement: C-M-, goes forward
`C-M-,` (`xref-go-forward`) SHALL take positions off the forward list, newest first, skipping as `M-,` does. It SHALL put the selected window's position on the back list without emptying the forward list, then show the position as `M-,` does. With nothing left it SHALL fail with `At end of xref history`.

#### Scenario: Forward undoes back
- **WHEN** at point 2 in `a.txt`, a `project-grep` hit in `b.txt` is chosen, then `M-,` and `C-M-,` are typed
- **THEN** `b.txt` shows with point at the hit

#### Scenario: Empty forward list
- **WHEN** `C-M-,` is typed with no history
- **THEN** `At end of xref history` shows
