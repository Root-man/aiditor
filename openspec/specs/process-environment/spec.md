# process-environment Specification

## Purpose

Gives each project the environment an interactive login shell in its primary repo has, so processes the editor starts for it find the user's tools even when the editor was not launched from a shell.

## Requirements

### Requirement: Launch check
When the editor's stdout is a terminal at launch, it SHALL capture no environment and every project SHALL use the editor's inherited environment. Otherwise it SHALL capture each project's environment.

#### Scenario: Launched from a shell
- **WHEN** the editor starts with stdout a terminal and a project's workspace opens
- **THEN** no shell is run and the project's environment is the editor's own

#### Scenario: Launched from the Dock
- **WHEN** the editor starts with stdout not a terminal and a project's first workspace opens
- **THEN** a capture starts for that project in its primary repo

### Requirement: Capturing a project's environment
A capture SHALL run the shell named by `SHELL` (or `/bin/sh` when unset) as `<shell> -l -i -c <command>` in the project's primary repo, off the UI thread, in a new session, with stdin from `/dev/null`. The command SHALL print a start marker, the environment with `env -0`, and an end marker; for fish it SHALL run `emit fish_prompt` first. Only the `NAME=value` entries between the markers SHALL be used, regardless of the shell's exit status and of any output before or after them. `SHLVL` SHALL be dropped. A capture SHALL start once per project per session, when the project first has a workspace, and the result SHALL be kept for the session.

#### Scenario: Profile noise is ignored
- **WHEN** the shell prints text before the start marker and after the end marker, and exits with status 1
- **THEN** the captured environment is exactly the entries between the markers, without `SHLVL`

#### Scenario: Captured once per project
- **WHEN** a second workspace opens on a project that has already been captured
- **THEN** no new capture starts

### Requirement: Fallback
A capture SHALL give up after 5 seconds, killing the shell's process group. When the capture times out, the shell cannot start, or its output lacks either marker, the project SHALL use the editor's own environment, and a line naming the project and the reason SHALL be added to `*Messages*`. A successful capture SHALL also be noted in `*Messages*`.

#### Scenario: A hanging shell
- **WHEN** the shell never exits
- **THEN** within the timeout its process group is killed, the project uses the editor's environment and `*Messages*` says why

#### Scenario: A failing shell
- **WHEN** the shell exits at once without printing the markers
- **THEN** the project uses the editor's environment and `*Messages*` says why

### Requirement: Re-capturing
`project-capture-environment` SHALL start a new capture for the current workspace's project. The project SHALL keep its previous environment while it runs; its result, or the fallback if it fails, SHALL replace that environment for processes started afterwards, and the answer of any earlier capture still running SHALL be ignored. When nothing is captured because the editor was launched from a shell, the command SHALL say so in the echo area and start nothing.

#### Scenario: A changed profile is picked up
- **WHEN** the shell's output changes after the first capture and `project-capture-environment` runs
- **THEN** the project's environment is the new output

#### Scenario: The old environment is kept meanwhile
- **WHEN** `getenv` runs while a re-capture of a captured project is running
- **THEN** it echoes the value from the previous capture

#### Scenario: A late answer is ignored
- **WHEN** an answer for the earlier capture arrives after the re-capture's answer
- **THEN** the project keeps the re-capture's environment

### Requirement: Reading a variable
`getenv` SHALL prompt `Get value of environment variable: ` and echo the variable's value in the current workspace project's environment: the captured one, or the editor's own when nothing is captured or the capture fell back. An unset variable SHALL echo `<name> not defined`; a capture still running SHALL echo that it is still running.

#### Scenario: A captured variable
- **WHEN** the current project's captured environment has `PATH=/a:/b` and `getenv` is answered `PATH`
- **THEN** the echo area shows `/a:/b`

#### Scenario: An unset variable
- **WHEN** `getenv` is answered with a name the environment lacks
- **THEN** the echo area shows `<name> not defined`
