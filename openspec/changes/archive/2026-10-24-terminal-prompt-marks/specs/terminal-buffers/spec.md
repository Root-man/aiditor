# Spec Delta

## ADDED Requirements

### Requirement: The terminal keeps prompt marks
A terminal SHALL record each OSC 133 mark it is fed, `A` (prompt start), `B` (input start), `C` (output start) and `D` (command end, with its exit code when one is given), at the absolute line of the cursor when the mark arrives, counted from the oldest scrollback line. Marks SHALL be kept in the order they arrived, whether the sequence ends with BEL or ST or is split across batches.

#### Scenario: A prompt, a command and its end
- **WHEN** `\e]133;A\a$ \e]133;B\als\r\n\e]133;C\aout\r\n\e]133;D;2\a` is fed into a terminal with an empty scrollback
- **THEN** its marks are `A` at line 0, `B` at line 0, `C` at line 1 and `D` with exit code 2 at line 2

#### Scenario: End without an exit code
- **WHEN** `\e]133;D\e\\` is fed
- **THEN** the terminal has one `D` mark with no exit code

#### Scenario: Lines that scrolled off keep their numbers
- **WHEN** a mark is fed on line 1 of a 3-row terminal and then five more lines scroll it into the scrollback
- **THEN** the mark is still at line 1

#### Scenario: Split across batches
- **WHEN** `\e]13` and `3;A\a` are fed as two batches
- **THEN** the terminal has one `A` mark

### Requirement: Marks are not kept on the alternate screen
A mark that arrives while the alternate screen is shown SHALL be ignored.

#### Scenario: A full-screen program prints a mark
- **WHEN** `\e[?1049h\e]133;A\a\e[?1049l` is fed
- **THEN** the terminal has no marks

### Requirement: Clearing the scrollback drops the marks
Erasing the scrollback on the main screen (ED 3, as `clear` sends), or a full reset (RIS) on either screen, SHALL drop every mark kept so far; marks fed after it are kept as usual.

#### Scenario: clear
- **WHEN** a mark is fed, then `\e[H\e[2J\e[3J`, then another mark
- **THEN** the terminal has only the second mark

### Requirement: A full scrollback stops the marks
Once the scrollback reaches its line limit, the terminal SHALL drop its marks and ignore new ones until the scrollback is cleared, since older lines no longer keep their numbers.

#### Scenario: Scrollback full
- **WHEN** a mark is fed and then more lines than the scrollback holds
- **THEN** the terminal has no marks, and a mark fed afterwards is ignored

### Requirement: The terminal keeps the shell's directory
A terminal SHALL take OSC 7's `file://host/path` or `kitty-shell-cwd://host/path` as its current directory, percent-decoding the path, when the host is empty, `localhost` or this machine's hostname and the decoded path is absolute. Any other OSC 7 SHALL leave the directory unchanged, since program output is untrusted. A terminal starts with no directory.

#### Scenario: Local host
- **WHEN** `\e]7;file://localhost/tmp/a%20b\a` is fed, then `\e]7;file:///home\e\\`
- **THEN** the directory is first `/tmp/a b`, then `/home`

#### Scenario: This machine
- **WHEN** OSC 7 names this machine's hostname with path `/srv`
- **THEN** the directory is `/srv`

#### Scenario: Remote, relative or garbage
- **WHEN** after `/home`, OSC 7 is fed with `file://elsewhere.example/etc`, `file://localhost` (no path), `file:tmp` (relative), `http://localhost/x` or `file:///bad%zz`
- **THEN** the directory stays `/home`
