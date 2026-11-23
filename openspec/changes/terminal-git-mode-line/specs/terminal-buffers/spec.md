## ADDED Requirements

### Requirement: The mode line shows the directory's git state
When the terminal's directory is inside a git repository, a terminal window's mode line SHALL show in its right group, after the running command or exit status, `⎇ <branch>`, named as a file's mode line names it, then `+<added> −<removed>`, the lines changed against `HEAD` with staged and unstaged changes together and untracked and binary files left out, shown only when either is non-zero, then `↑<ahead> ↓<behind>`, the commits between the branch and its upstream as last fetched, each part shown only when non-zero. Outside a repository, with no directory, or when git can't answer, no git segment SHALL show. When the repository's own config defines a content filter, the line counts SHALL not show, since computing them would run the filter.

#### Scenario: A clean branch
- **WHEN** the terminal's directory is in a repository on `main` with no changes and no upstream
- **THEN** the right group is `⎇ main` then `char`

#### Scenario: Changed lines
- **WHEN** a tracked file has 2 lines added and 1 removed, part staged
- **THEN** the right group holds `⎇ main` then `+2 −1`

#### Scenario: Ahead of the upstream
- **WHEN** the branch is 2 commits ahead of its upstream and level behind
- **THEN** the upstream segment is `↑2`

#### Scenario: Diverged
- **WHEN** the branch is 2 ahead and 1 behind
- **THEN** the upstream segment is `↑2 ↓1`

#### Scenario: Outside a repository
- **WHEN** the terminal's directory isn't in a repository
- **THEN** no git segment shows

#### Scenario: A filtering repository
- **WHEN** the repository's config defines `filter.lfs.clean` and a file is changed
- **THEN** `⎇ <branch>` shows and no line count does

### Requirement: The terminal's git state follows the shell
A terminal SHALL ask for its directory's git state when it is made, when its directory changes, when a command ends with a `D` mark, and when the watcher reports a change under the repository or to its `HEAD` or index. An answer for a directory the terminal has since left SHALL be dropped. When the directory moves out of the repository, its segments SHALL disappear at once rather than wait for the answer.

#### Scenario: Changing directory
- **WHEN** OSC 7 moves the shell from one repository to another
- **THEN** the mode line shows the second repository's branch once it answers

#### Scenario: A command ends
- **WHEN** a command commits the changes and its `D` mark arrives
- **THEN** the line counts are asked again and disappear

#### Scenario: Leaving the repository
- **WHEN** OSC 7 moves the shell from a repository to a directory outside it
- **THEN** the git segments are gone before the file service answers, while a move within the repository keeps them

#### Scenario: A stale answer
- **WHEN** an answer arrives for a directory the shell has left
- **THEN** the mode line keeps the current directory's state

#### Scenario: Branch switched elsewhere
- **WHEN** another terminal switches the repository's branch and the watcher reports its `HEAD`
- **THEN** this terminal's mode line names the new branch

### Requirement: Git segments give way on a narrow mode line
When a terminal's mode line doesn't fit its window, its git segments SHALL be dropped, the upstream first, then the line counts, then the branch, before the directory is shortened.

#### Scenario: Room for all but the upstream
- **WHEN** the mode line is one column too wide with all three git segments
- **THEN** the upstream segment is dropped and the directory is whole

## MODIFIED Requirements

### Requirement: The terminal follows the running command
A terminal SHALL take a `C` mark's `cmdline_url=` field, percent-decoded, as its running command. A `C` mark SHALL set the running command, none if the field is absent or malformed, and clear the exit status. A `D` mark SHALL clear the running command and set the exit status to its code when that is not zero, else clear it. An `A` mark SHALL clear the running command. Both SHALL follow marks on either screen and past a full scrollback.

#### Scenario: A command runs and fails
- **WHEN** `\e]133;C;cmdline_url=cargo%20test\a` is fed, then `\e]133;D;101\a`
- **THEN** after a second the right group holds `◉ cargo test` and `char`, then `exit 101` and `char`

#### Scenario: The next command clears the failure
- **WHEN** a command exited 101 and a `C` mark starts another
- **THEN** `exit 101` is gone while it runs, and stays gone if it exits 0

#### Scenario: An empty prompt keeps the failure
- **WHEN** a command exited 101 and an `A` mark arrives with no `C` mark
- **THEN** `exit 101` stays

#### Scenario: No command text
- **WHEN** `\e]133;C\a` or `\e]133;C;cmdline_url=%zz\a` is fed
- **THEN** no running command shows

#### Scenario: A shell that sends no end
- **WHEN** a running command is followed by an `A` mark with no `D`
- **THEN** the running command is gone

#### Scenario: A full scrollback
- **WHEN** the scrollback is full and marks are no longer kept, and `C` then `D` with a non-zero code are fed
- **THEN** the running command and exit status still follow them

#### Scenario: Ending on the alternate screen
- **WHEN** a program that was running leaves the terminal on the alternate screen and the `D` mark arrives there
- **THEN** the running command is gone and the exit status shows

### Requirement: The mode line shows the running command and the last failure
A terminal window's mode line SHALL show `◉ <command>`, with control chars as spaces, once the command has run for one second, so a quick command never shows, and `exit <n>`, as the right group's first segments, left of the git segments and the input mode. A command that has run a second SHALL show even when it writes nothing.

#### Scenario: A multi-line command
- **WHEN** the running command's field is `a%0Ab`
- **THEN** the segment is `◉ a b`

#### Scenario: Copy mode
- **WHEN** a command exited 101 and copy mode is entered with point on line 5001, column 1
- **THEN** the right group's segments are `exit 101`, `copy` and `5001:1`

#### Scenario: A quick command
- **WHEN** a command's `C` mark is followed by its `D` mark with exit code 0 half a second later
- **THEN** no `◉` segment shows at any point

#### Scenario: A silent long command
- **WHEN** `sleep 5` starts in a repository on `main` and writes nothing for a second
- **THEN** the right group is `◉ sleep 5`, `⎇ main` and `char`
