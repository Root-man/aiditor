# Spec Delta

## MODIFIED Requirements

### Requirement: Killing a terminal
Killing a terminal SHALL send SIGHUP, then SIGTERM, to its shell's process group and to its PTY's foreground process group, and SIGKILL to both 100 ms later, even if the shell has exited by then, so a job ignoring the hangup does not outlive it. Killing SHALL return to the caller at once and free the terminal's input queue, so later input and resizes for it are ignored. A terminal killed before its shell has started SHALL be killed as soon as the shell starts. The core SHALL still get the terminal's exit message. A kill of an unknown terminal SHALL be ignored. Killing a terminal whose shell has already exited, or failed to start, SHALL send no signal and SHALL release its PTY and input thread.

#### Scenario: The shell and its foreground job are gone
- **WHEN** a terminal running an interactive shell with `sleep 1000` as its foreground job is killed
- **THEN** within seconds neither the shell nor the `sleep` process exists, and the core gets the exit message

#### Scenario: A shell that traps SIGHUP and SIGTERM
- **WHEN** a terminal runs a shell that ignores SIGHUP and SIGTERM, and is killed
- **THEN** within seconds the shell no longer exists

#### Scenario: Killed before it started
- **WHEN** a terminal is killed right after it is spawned
- **THEN** within seconds its shell no longer exists

#### Scenario: Killed after its shell exited
- **WHEN** twelve terminals run `/bin/sh -c 'exit 0'`, each is killed after its exit message arrives
- **THEN** the process's open file descriptors return to their count before the spawns, without killing all terminals

#### Scenario: Killed after a failed spawn
- **WHEN** a terminal is spawned in a cwd that is not a directory, and is killed after its spawn failure arrives
- **THEN** no signal is sent and the service holds nothing for that terminal
