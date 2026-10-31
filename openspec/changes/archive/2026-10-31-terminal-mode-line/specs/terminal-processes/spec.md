## MODIFIED Requirements

### Requirement: Zsh sends prompt marks and its directory
In an interactive integrated zsh, the shell SHALL send `OSC 133;D;<status>` after a command, `OSC 133;A` and `OSC 7` with its directory before each prompt, `OSC 133;B` once at the end of each prompt (from the second prompt when a hook rewrites the prompt) and `OSC 133;C;cmdline_url=<command>` before a command runs, the command line as typed, percent-encoded byte by byte. These hooks SHALL run after the user's own prompt hooks without changing the exit status those hooks see.

#### Scenario: Marks around a command
- **WHEN** zsh is installed, a terminal with an integration folder is spawned with no command and `SHELL` set to zsh, and `false` is typed and run
- **THEN** the output has `133;A`, an `OSC 7` with the current directory, and `133;B` before the first prompt, then `133;C;cmdline_url=false`, then `133;D;1`, `133;A` and `OSC 7` before the next prompt

#### Scenario: Hooks run after the prompt's own hooks
- **WHEN** the user's `.zshrc` adds a precmd hook that sets `PS1` and records `$?`
- **THEN** the hook still sees the command's exit status, and `133;B` appears once at the end of every prompt after the first, whose hooks ran before the integration's moved to the end

#### Scenario: A command with spaces and non-ASCII
- **WHEN** `true é;` is typed and run
- **THEN** the `C` mark's field is `cmdline_url=true%20%C3%A9%3B`
