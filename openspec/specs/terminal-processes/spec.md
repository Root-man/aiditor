# terminal-processes Specification

## Purpose

Runs the user's shell, or another command, in a pseudo-terminal off the UI thread and streams its output and exit status to the core, with flow control that keeps one terminal's flood from stalling the rest of the editor.

## Requirements

### Requirement: Spawning a terminal
A terminal SHALL be spawned from a terminal id chosen by the core, a working directory, extra environment variables and a size in rows and columns, running either a given command or, by default, the user's shell. Spawning SHALL NOT block the UI thread. The core SHALL get one message saying the terminal started, or why it could not start. A working directory that is not a directory, and a command that cannot be found or run, SHALL be spawn failures.

#### Scenario: A command starts
- **WHEN** a terminal is spawned with `/bin/sh -c 'echo hi'` in an existing directory
- **THEN** the core gets a started message for that id before any output from it

#### Scenario: A missing program
- **WHEN** a terminal is spawned with a command `/nonexistent/program`
- **THEN** the core gets a failure message for that id with the reason, and no output or exit message

#### Scenario: A missing working directory
- **WHEN** a terminal is spawned with a working directory that does not exist
- **THEN** the core gets a failure message for that id, and no shell starts in another directory

### Requirement: The default command is a login shell
With no command given, the terminal SHALL run the shell named by `SHELL` in the terminal's environment as a login shell, its argv0 being `-` followed by the shell's file name, so the shell reads its login profile.

#### Scenario: Bash as a login shell
- **WHEN** a terminal is spawned with no command, `SHELL=/bin/bash`, and `HOME` set to a directory whose `.bash_profile` prints `$0` and exits
- **THEN** the output is `-bash`

#### Scenario: Zsh as a login shell
- **WHEN** zsh is installed and a terminal is spawned with no command, `SHELL` set to zsh, and `ZDOTDIR` set to a directory whose `.zprofile` prints `$0` and exits
- **THEN** the output is `-zsh`

### Requirement: The terminal's environment
The terminal's process SHALL inherit the editor's environment with the spawn's extra variables on top, and SHALL get `TERM=xterm-256color`, `COLORTERM=truecolor`, `TERM_PROGRAM=aiditor` and `TERM_PROGRAM_VERSION` set to the editor's version. `LANG` SHALL be `en_US.UTF-8` when neither the editor's environment nor the extra variables set it. `SHLVL` SHALL be removed, so the shell starts at level 1.

#### Scenario: Terminal variables
- **WHEN** a terminal runs `env`
- **THEN** the output has `TERM=xterm-256color`, `COLORTERM=truecolor`, `TERM_PROGRAM=aiditor`, `TERM_PROGRAM_VERSION=<the editor's version>`, and no `SHLVL` other than one the shell sets itself

#### Scenario: LANG default
- **WHEN** the editor's environment has no `LANG` and a terminal runs `echo $LANG`
- **THEN** the output is `en_US.UTF-8`

#### Scenario: Extra variables
- **WHEN** a terminal is spawned with extra variable `FOO=bar` and runs `echo $FOO`
- **THEN** the output is `bar`

### Requirement: UTF-8 line editing
The terminal's PTY SHALL have the `IUTF8` input flag set on macOS and Linux, so the kernel's canonical-mode line editing treats a multibyte UTF-8 char as one char.

#### Scenario: The flag is on
- **WHEN** a terminal runs `stty -a`
- **THEN** its output lists `iutf8` without a leading `-`

#### Scenario: Backspace deletes a whole multibyte char
- **WHEN** a terminal in canonical mode runs `read -r line; printf '<%s>' "$line"` and gets `aé`, the erase char `\x7f` and `\n`
- **THEN** its output holds `<a>`

### Requirement: Output and exit stream to the core
The terminal's output SHALL reach the core as byte batches of at most 64 KB, in the order it was written, each tagged with the terminal id. When the process exits, the core SHALL get one exit message with its exit code, or the signal that ended it. Output the process wrote before exiting SHALL arrive before the exit message. The exit SHALL be reported when the process exits, even if a background job it started keeps the PTY open and keeps writing.

#### Scenario: Output then exit code
- **WHEN** a terminal runs `/bin/sh -c 'printf a; printf b; exit 3'`
- **THEN** the core gets output `ab`, then an exit message with code 3

#### Scenario: A large write is batched
- **WHEN** a terminal writes 1 MB of output
- **THEN** every output message holds at most 64 KB, and together they hold all the output in order

#### Scenario: A background job holds the PTY
- **WHEN** a terminal runs `/bin/sh -c 'sleep 5 & exit 0'`
- **THEN** the exit message with code 0 arrives well before the `sleep` ends

### Requirement: Flow control per terminal
Each terminal SHALL send to the core on its own bounded channel, a few batches deep. When the core does not drain a terminal's channel, that terminal SHALL stop reading once its channel is full, so the producing program blocks once the PTY's buffer fills. Other terminals and services SHALL still reach the core.

#### Scenario: An undrained terminal blocks its producer
- **WHEN** a terminal runs `/bin/sh -c 'head -c 8000000 /dev/zero; touch done'` and its channel is never drained
- **THEN** `done` does not appear, and a file service reply sent meanwhile still reaches the core

#### Scenario: Draining resumes the producer
- **WHEN** the channel of that blocked terminal is then drained
- **THEN** all 8,000,000 bytes arrive and `done` appears

### Requirement: Writing input
Bytes written to a terminal SHALL reach its PTY unchanged and in the order they were written. Writing SHALL return to the caller at once, without waiting for the PTY to accept the bytes, so a write larger than the PTY's input buffer to a program that doesn't read never blocks the UI thread. The PTY's line discipline SHALL act on the bytes as on a real terminal's input: echo, line editing and signal characters. A write to an unknown terminal, or to one whose process has gone, SHALL be ignored.

#### Scenario: Input echoes back
- **WHEN** a terminal runs `cat` and `hi\n` is written to it, followed by `\x04`
- **THEN** its output holds `hi` twice, the echo and `cat`'s copy, then `cat` exits with code 0

#### Scenario: Ctrl-C interrupts the foreground job
- **WHEN** a terminal runs `sleep 30` and `\x03` is written to it
- **THEN** the exit message arrives within seconds, and its status is not a clean exit

#### Scenario: A large write to a program that doesn't read
- **WHEN** a terminal runs a program in raw mode that never reads, and 1 MB is written to it
- **THEN** the write returns at once

### Requirement: Resizing
A terminal SHALL be resizable to a number of rows and columns, setting its PTY's window size, so programs in it see the new size and the foreground job gets `SIGWINCH`. A resize SHALL take effect before any bytes written after it are read by the program. A resize of an unknown terminal SHALL be ignored.

#### Scenario: stty sees the new size
- **WHEN** a terminal spawned at 24 by 80 runs `read x; stty size`, is resized to 30 rows and 100 columns, and then gets `\n`
- **THEN** its output holds `30 100`

### Requirement: Killing a terminal
Killing a terminal SHALL send SIGHUP, then SIGTERM, to its shell's process group and to its PTY's foreground process group, and SIGKILL to both 100 ms later, even if the shell has exited by then, so a job ignoring the hangup does not outlive it. Killing SHALL return to the caller at once and free the terminal's input queue, so later input and resizes for it are ignored. A terminal killed before its shell has started SHALL be killed as soon as the shell starts. The core SHALL still get the terminal's exit message. A kill of an unknown terminal SHALL be ignored.

#### Scenario: The shell and its foreground job are gone
- **WHEN** a terminal running an interactive shell with `sleep 1000` as its foreground job is killed
- **THEN** within seconds neither the shell nor the `sleep` process exists, and the core gets the exit message

#### Scenario: A shell that traps SIGHUP and SIGTERM
- **WHEN** a terminal runs a shell that ignores SIGHUP and SIGTERM, and is killed
- **THEN** within seconds the shell no longer exists

#### Scenario: Killed before it started
- **WHEN** a terminal is killed right after it is spawned
- **THEN** within seconds its shell no longer exists

### Requirement: No shells left behind on quit
When the app quits, every live terminal SHALL be killed before the process exits, waiting once for the 100 ms grace before SIGKILL, also when a terminal is still starting, so its thread can start and kill it.

#### Scenario: Quitting kills every terminal
- **WHEN** two terminals are running, one of them ignoring SIGHUP and SIGTERM, and all terminals are killed for quitting
- **THEN** it returns after the grace, with SIGKILL already sent, and within seconds neither shell exists

#### Scenario: Quitting right after a spawn
- **WHEN** a terminal is spawned and all terminals are killed for quitting at once
- **THEN** within seconds its shell no longer exists
