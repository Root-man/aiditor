## ADDED Requirements

### Requirement: Requests open their files
A request for a live terminal SHALL open each of its files in that terminal's workspace. It SHALL reuse that workspace's buffer visiting the file, or else read the file into a new buffer, empty when there is no file. Each buffer SHALL get the `server-edit` minor mode. A file that can't be read, or isn't text, SHALL abort the client with the reason.

#### Scenario: git commit opens COMMIT_EDITMSG
- **WHEN** a terminal's client asks to edit `/r/.git/COMMIT_EDITMSG`, and the window showing the terminal is selected
- **THEN** that window shows a buffer visiting the file, with `server-edit` on

#### Scenario: An open buffer is reused
- **WHEN** a client asks to edit a file that a buffer of the terminal's workspace already visits
- **THEN** that buffer is shown, with `server-edit` on, and no second buffer is made

#### Scenario: A position
- **WHEN** a client asks to edit a file at line 2, column 3
- **THEN** point is on its second line, after two characters

#### Scenario: An unreadable file
- **WHEN** a requested file can't be read
- **THEN** the client gets code 1 with the read error

### Requirement: Where the files show
Once every file is open, the first SHALL be shown in the workspace's window showing the terminal, selected when the workspace is current, or else in its selected window. A file with a line, and optionally a column, both from 1, SHALL have point there. When the terminal's workspace isn't current, the echo area SHALL say that an edit waits there.

#### Scenario: Another workspace
- **WHEN** a client's terminal belongs to a workspace that isn't current
- **THEN** the current windows don't change and the echo area names the file and the workspace

### Requirement: Finishing an edit
`server-edit`, bound to `C-c C-c` and `C-x #` in `server-edit`, SHALL finish the current buffer's edit. A modified buffer SHALL be saved first, and the edit finishes only once that save succeeds; a failed save SHALL leave the edit waiting. A finished buffer SHALL lose `server-edit`. When every file of a client is finished, the client SHALL get code 0.

#### Scenario: Finishing commits
- **WHEN** the user edits a waiting COMMIT_EDITMSG and presses `C-c C-c`
- **THEN** the buffer is saved, and once the save succeeds the client gets code 0

#### Scenario: A failed save
- **WHEN** the save started by `C-c C-c` fails
- **THEN** the client gets no reply and the buffer keeps `server-edit`

#### Scenario: Two files
- **WHEN** a client waits on two files and the first is finished
- **THEN** the client gets no reply and the window shows the second, until that one is finished too

### Requirement: Aborting an edit
`server-edit-abort`, bound to `C-c C-k` in `server-edit`, SHALL give the current buffer's client code 1 with the error `Edit aborted` without saving. Killing a buffer the client waits on, or its terminal's buffer, SHALL give the client code 1 too. So SHALL killing the workspace, which kills both. Quitting SHALL close the connection, so the client exits 1. Its other buffers SHALL lose `server-edit`.

#### Scenario: Aborting leaves no commit
- **WHEN** the user presses `C-c C-k` in a waiting COMMIT_EDITMSG
- **THEN** the client gets code 1, and nothing is saved

#### Scenario: Killing the buffer
- **WHEN** the user kills a buffer a client waits on
- **THEN** the client gets code 1

#### Scenario: Killing the terminal
- **WHEN** the user kills the terminal buffer whose client waits
- **THEN** the client gets code 1

#### Scenario: Quitting
- **WHEN** the user quits while a client waits
- **THEN** the quit asks first, since the terminal is a live process, and the client exits 1 once the editor has gone

### Requirement: After an edit
After a buffer's edit is finished or aborted, the window that showed it SHALL show the client's next waiting file, or else the terminal. A buffer opened for the edit SHALL be killed; one that was already open SHALL stay.

#### Scenario: Back to the terminal
- **WHEN** the edit of a file opened for it is finished
- **THEN** the window shows the terminal and the file's buffer is gone

#### Scenario: A buffer that was open stays
- **WHEN** the edit of a file that was already open is aborted
- **THEN** the window shows the terminal and the buffer is still there, without `server-edit`

### Requirement: A client that goes away
When a client's connection closes before its reply, the server SHALL tell the core. The core SHALL drop the client. Its buffers SHALL stay but lose `server-edit`.

#### Scenario: ^C in the terminal
- **WHEN** the client is interrupted while its file is open
- **THEN** the buffer stays and `server-edit` is off

## REMOVED Requirements

### Requirement: Editing waits for the finish and abort keys
**Reason**: Requests now open their files, and finishing or aborting the edit answers them.
**Migration**: None. Clients now wait for the edit instead of failing at once.
