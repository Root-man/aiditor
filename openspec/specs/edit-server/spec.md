# edit-server Specification

## Purpose
Lets a program running in a terminal buffer, such as git, hand files to the running editor and block until the editor answers, with `aiditor --wait` as the client.

## Requirements

### Requirement: The editor's socket
Each editor process SHALL listen on a Unix socket at `aiditor/<pid>.sock` under `$XDG_RUNTIME_DIR` on Linux and the per-user temporary directory (`$TMPDIR`) on macOS. The directory SHALL be mode 0700 and the socket 0600. A stale socket at that path SHALL be replaced. On quit the socket SHALL be removed. On Linux without `$XDG_RUNTIME_DIR`, or when listening fails, the editor SHALL run without a socket and log a warning.

#### Scenario: Permissions
- **WHEN** the editor starts listening
- **THEN** the socket file is mode 0600 and its directory is mode 0700

#### Scenario: Gone after quit
- **WHEN** the editor quits
- **THEN** the socket file no longer exists

### Requirement: The client
`aiditor --wait [+LINE[:COL]] FILE...` SHALL run as the client, before any window opens. It SHALL send one request line with its working directory, the terminal id from `AIDITOR_TERMINAL` and its arguments to the socket in `AIDITOR_SOCKET`, then block for one reply line and exit with that reply's code, printing the reply's error, if any, to stderr.

#### Scenario: A successful reply
- **WHEN** the client runs and the editor replies with code 0
- **THEN** the client exits 0

#### Scenario: A refusal
- **WHEN** the client runs and the editor replies with code 1 and an error
- **THEN** the client prints the error to stderr and exits 1

### Requirement: The client without an editor
The client SHALL exit 2 with the reason on stderr when `AIDITOR_SOCKET` or `AIDITOR_TERMINAL` is unset or invalid, or when it cannot connect. It SHALL exit 1 with a message on stderr when the connection closes before a reply arrives.

#### Scenario: No socket variable
- **WHEN** the client runs without `AIDITOR_SOCKET`
- **THEN** it prints an error and exits 2

#### Scenario: A dead socket
- **WHEN** `AIDITOR_SOCKET` names a path nothing listens on
- **THEN** the client prints an error and exits 2

#### Scenario: The editor goes away
- **WHEN** the connection closes before a reply
- **THEN** the client prints an error and exits 1

### Requirement: Requests reach the core
The server SHALL turn a well-formed request into one message to the core. The message SHALL carry a request number, the terminal id, and each file as an absolute path, resolving relative paths against the client's working directory, with the line and column from a `+LINE` or `+LINE:COL` argument just before it. The core's reply for that request number SHALL be written back on the same connection.

#### Scenario: A request with a position
- **WHEN** a client in `/tmp/w` sends `+3:5 a.txt b.txt` for terminal 7
- **THEN** the core gets a request for terminal 7 with `/tmp/w/a.txt` at line 3, column 5 and `/tmp/w/b.txt` with no position

#### Scenario: The reply goes back
- **WHEN** the core replies to a request
- **THEN** that request's client receives the reply and exits with its code

### Requirement: Requests are untrusted
The server SHALL refuse, with code 1 and an error, a request line that is not valid JSON, is longer than its limit, has a relative working directory, or names no file. It SHALL refuse it without messaging the core and without affecting other connections. The core SHALL refuse a request for a terminal it does not know.

#### Scenario: Malformed request
- **WHEN** a client sends `not json`
- **THEN** it gets code 1 with an error, the core gets no message, and the server still answers later requests

#### Scenario: Unknown terminal
- **WHEN** the core gets a request for a terminal id it has no terminal for
- **THEN** it replies code 1 with an error naming the terminal

### Requirement: Editing waits for the finish and abort keys
Until finishing and aborting edits exists, the core SHALL refuse a request for a known terminal with code 1 and the error "Editing from terminals isn't built yet".

#### Scenario: A known terminal
- **WHEN** the core gets a request for a live terminal
- **THEN** it replies code 1 with that error
