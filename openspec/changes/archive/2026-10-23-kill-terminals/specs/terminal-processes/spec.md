# Spec Delta

## ADDED Requirements

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
