## MODIFIED Requirements

### Requirement: The mode line shows the terminal's mode
A terminal window's mode line SHALL show the input mode as a right group segment: `char` in char mode, without point's position, and `copy` followed by a segment with point's line and column in copy mode.

#### Scenario: Char mode
- **WHEN** a terminal buffer `t` is shown in char mode, with no directory
- **THEN** its left group is `t` and its right group `char`

#### Scenario: Copy mode
- **WHEN** copy mode is entered with point on line 5001, column 1
- **THEN** the left group is `t` and the right group's segments are `copy` and `5001:1`

## ADDED Requirements

### Requirement: The mode line shows the shell's directory
A terminal window's mode line SHALL show the terminal's directory as the left group's second segment, with the home directory as `~`, and no segment when the terminal has no directory.

#### Scenario: Under home
- **WHEN** the terminal's directory is `crates` under the home directory
- **THEN** the segment is `~/crates`

#### Scenario: Home itself
- **WHEN** the terminal's directory is the home directory
- **THEN** the segment is `~`

#### Scenario: Elsewhere
- **WHEN** OSC 7 sets the directory to `/srv`
- **THEN** the segment is `/srv`

#### Scenario: Too narrow
- **WHEN** the directory is `/srv/one/two/three` and the left group has room for 14 columns
- **THEN** it reads `t │ …two/three`, keeping the directory's end

#### Scenario: No room for the directory
- **WHEN** the left group has room for no part of the directory
- **THEN** it reads `t`, with no separator

### Requirement: The terminal follows the running command
A terminal SHALL take a `C` mark's `cmdline_url=` field, percent-decoded, as its running command. A `C` mark SHALL set the running command, none if the field is absent or malformed, and clear the exit status. A `D` mark SHALL clear the running command and set the exit status to its code when that is not zero, else clear it. An `A` mark SHALL clear the running command. Both SHALL follow marks on either screen and past a full scrollback.

#### Scenario: A command runs and fails
- **WHEN** `\e]133;C;cmdline_url=cargo%20test\a` is fed, then `\e]133;D;101\a`
- **THEN** the right group holds `◉ cargo test` and `char`, then `exit 101` and `char`

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
A terminal window's mode line SHALL show `◉ <command>`, with control chars as spaces, and `exit <n>` as right group segments before the input mode.

#### Scenario: A multi-line command
- **WHEN** the running command's field is `a%0Ab`
- **THEN** the segment is `◉ a b`

#### Scenario: Copy mode
- **WHEN** a command exited 101 and copy mode is entered with point on line 5001, column 1
- **THEN** the right group's segments are `exit 101`, `copy` and `5001:1`
