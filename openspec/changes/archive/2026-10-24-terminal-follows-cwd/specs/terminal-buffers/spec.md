# Spec Delta

## MODIFIED Requirements

### Requirement: vterm opens a terminal buffer
`vterm` (through `M-x`) SHALL spawn the user's login shell in a new terminal buffer of the current workspace and show it in the selected window. The buffer SHALL be named `vterm<n><workspace>`, `n` the smallest number from 1 no buffer of that name has. The shell's working directory SHALL be the current buffer's file's directory, a terminal buffer's directory, or the primary root of the current workspace's project when the buffer has neither.

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

#### Scenario: Working directory from a terminal
- **WHEN** a terminal's shell reported `/tmp` through OSC 7 and `M-x vterm` runs from its buffer
- **THEN** the new shell is spawned in `/tmp`

#### Scenario: Working directory from a terminal without OSC 7
- **WHEN** a terminal spawned in `/r/app` reported no directory and `M-x vterm` runs from its buffer
- **THEN** the new shell is spawned in `/r/app`

### Requirement: The terminal keeps the shell's directory
A terminal SHALL take OSC 7's `file://host/path` or `kitty-shell-cwd://host/path` as its current directory, percent-decoding the path, when the host is empty, `localhost` or this machine's hostname and the decoded path is absolute. Any other OSC 7 SHALL leave the directory unchanged, since program output is untrusted. A terminal starts with the directory its shell is spawned in.

#### Scenario: Local host
- **WHEN** `\e]7;file://localhost/tmp/a%20b\a` is fed, then `\e]7;file:///home\e\\`
- **THEN** the directory is first `/tmp/a b`, then `/home`

#### Scenario: This machine
- **WHEN** OSC 7 names this machine's hostname with path `/srv`
- **THEN** the directory is `/srv`

#### Scenario: Remote, relative or garbage
- **WHEN** after `/home`, OSC 7 is fed with `file://elsewhere.example/etc`, `file://localhost` (no path), `file:tmp` (relative), `http://localhost/x` or `file:///bad%zz`
- **THEN** the directory stays `/home`

#### Scenario: Before any OSC 7
- **WHEN** `M-x vterm` spawns a shell in `/r/app` and no OSC 7 has been fed
- **THEN** the terminal's directory is `/r/app`
