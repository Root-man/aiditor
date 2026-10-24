## ADDED Requirements

### Requirement: Zsh starts with aiditor's integration
When the terminal runs the default shell, the shell is zsh and the editor has an integration folder, zsh SHALL start with `ZDOTDIR` set to that folder. Its `.zshenv` SHALL restore the user's `ZDOTDIR`, or unset it if it was unset, and source the user's `.zshenv`, so zsh reads the user's other startup files. Other shells and given commands SHALL start unchanged.

#### Scenario: The user's startup files and ZDOTDIR are kept
- **WHEN** the user's `ZDOTDIR` names a directory whose `.zshenv` and `.zshrc` set variables, and the terminal runs `echo $ZDOTDIR` and those variables
- **THEN** the output is the user's `ZDOTDIR` and both variables

#### Scenario: An unset ZDOTDIR stays unset
- **WHEN** the editor's environment has no `ZDOTDIR` and the terminal runs `echo ${ZDOTDIR-unset}`
- **THEN** the output is `unset`

#### Scenario: Bash is unchanged
- **WHEN** a terminal with an integration folder is spawned with no command and `SHELL=/bin/bash`
- **THEN** bash starts without `ZDOTDIR` set by the editor

### Requirement: Zsh sends prompt marks and its directory
In an interactive integrated zsh, the shell SHALL send `OSC 133;D;<status>` after a command, `OSC 133;A` and `OSC 7` with its directory before each prompt, `OSC 133;B` once at the end of each prompt (from the second prompt when a hook rewrites the prompt) and `OSC 133;C` before a command runs. These hooks SHALL run after the user's own prompt hooks without changing the exit status those hooks see.

#### Scenario: Marks around a command
- **WHEN** zsh is installed, a terminal with an integration folder is spawned with no command and `SHELL` set to zsh, and `false` is typed and run
- **THEN** the output has `133;A`, an `OSC 7` with the current directory, and `133;B` before the first prompt, then `133;C`, then `133;D;1`, `133;A` and `OSC 7` before the next prompt

#### Scenario: Hooks run after the prompt's own hooks
- **WHEN** the user's `.zshrc` adds a precmd hook that sets `PS1` and records `$?`
- **THEN** the hook still sees the command's exit status, and `133;B` appears once at the end of every prompt after the first, whose hooks ran before the integration's moved to the end
