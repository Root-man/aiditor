## MODIFIED Requirements

### Requirement: Switching the selected window to a buffer
`switch-to-buffer` (`C-x b`) SHALL read a buffer name in the minibuffer, completing over every buffer of the current workspace, and `*Messages*`, whose name does not start with a space: the other buffers most recently shown first, then the current buffer. It SHALL show the named buffer in the selected window and make it the most recent. A name no such buffer has SHALL create an empty buffer of that name in the current workspace and show it, even when another workspace has a buffer of that name.

#### Scenario: Back to the previous buffer
- **WHEN** a file is visited from `*scratch*` and `C-x b RET` is typed
- **THEN** `*scratch*` is the current buffer

#### Scenario: Most recent first
- **WHEN** files `a` then `b` are visited from `*scratch*` and `C-x b` is typed
- **THEN** the list holds `a`, `*scratch*`, `b` in that order

#### Scenario: A new name makes a buffer
- **WHEN** `C-x b notes RET` is typed
- **THEN** an empty buffer named `notes` is current

#### Scenario: Hidden buffers are not listed
- **WHEN** `C-x b` is typed
- **THEN** no listed name starts with a space

#### Scenario: Another workspace's buffers are not listed
- **WHEN** file `a` is visited in one workspace, a new workspace is made, and `C-x b` is typed
- **THEN** the list holds the new workspace's `*scratch*` and `*Messages*`, and not `a` or the first workspace's `*scratch*`

#### Scenario: Another workspace's name makes a new buffer
- **WHEN** file `a` is visited in one workspace, a new workspace is made, and `C-x b a RET` is typed
- **THEN** an empty buffer named `a<2>` of the new workspace is current, and the first workspace's `a` is unchanged

### Requirement: Killing a buffer
`kill-buffer` (`C-x k`) SHALL read a buffer name, completing over the listed buffers of the current workspace and `*Messages*` with the current one first, and kill that buffer. Every window showing it SHALL show the most recent other listed buffer of the current workspace, or `*Messages*`, instead. A buffer of another workspace SHALL NOT replace it. A name no such buffer has, another workspace's buffer included, SHALL fail with `No such buffer <name>`. `*Messages*` and each workspace's `*scratch*` SHALL NOT be killed: the kill SHALL fail with `Can't kill <name>`.

#### Scenario: Kill the current buffer
- **WHEN** a file is visited from `*scratch*` and `C-x k RET` is typed
- **THEN** the file's buffer is gone and `*scratch*` is current

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
