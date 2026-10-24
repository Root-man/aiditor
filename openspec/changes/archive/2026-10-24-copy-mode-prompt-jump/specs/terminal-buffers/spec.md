## MODIFIED Requirements

### Requirement: The terminal keeps prompt marks
A terminal SHALL record each OSC 133 mark it is fed, `A` (prompt start), `B` (input start), `C` (output start) and `D` (command end, with its exit code when one is given), at the absolute line and column of the cursor when the mark arrives, the line counted from the oldest scrollback line. Marks SHALL be kept in the order they arrived, whether the sequence ends with BEL or ST or is split across batches.

#### Scenario: A prompt, a command and its end
- **WHEN** `\e]133;A\a$ \e]133;B\als\r\n\e]133;C\aout\r\n\e]133;D;2\a` is fed into a terminal with an empty scrollback
- **THEN** its marks are `A` at line 0 column 0, `B` at line 0 column 2, `C` at line 1 column 0 and `D` with exit code 2 at line 2 column 0

#### Scenario: End without an exit code
- **WHEN** `\e]133;D\e\\` is fed
- **THEN** the terminal has one `D` mark with no exit code

#### Scenario: Lines that scrolled off keep their numbers
- **WHEN** a mark is fed on line 1 of a 3-row terminal and then five more lines scroll it into the scrollback
- **THEN** the mark is still at line 1

#### Scenario: Split across batches
- **WHEN** `\e]13` and `3;A\a` are fed as two batches
- **THEN** the terminal has one `A` mark

## ADDED Requirements

### Requirement: Copy mode jumps between prompts
In copy mode, `C-c C-p` (`vterm-previous-prompt`) SHALL move point to the previous command's input start before point, and `C-c C-n` (`vterm-next-prompt`) to the next one after point. A command's input start SHALL be its `B` mark, else its `A` mark, at the place in copy mode's text where the terminal had it, counting soft-wrapped rows joined into one line. With none in that direction, the command SHALL echo `No prompt found` and leave point.

#### Scenario: Back and forth over three commands
- **WHEN** three commands have run under OSC 133 marks, copy mode is entered at the fourth prompt and `C-c C-p` is pressed three times
- **THEN** point lands on the third, second and first command's input start in turn, and `C-c C-n` goes back to the second

#### Scenario: A wrapped command line
- **WHEN** a prompt's input starts on the second row of a soft-wrapped line
- **THEN** `C-c C-p` lands on that char in the joined line

#### Scenario: No marks
- **WHEN** `C-c C-p` is pressed in copy mode of a terminal with no marks
- **THEN** `No prompt found` is echoed and point stays

### Requirement: Prompt jumps take a count
A prefix count of n SHALL make a prompt jump move n commands, a negative count the other way, stopping at the farthest when fewer exist.

#### Scenario: A count skips prompts
- **WHEN** `C-u 2 C-c C-p` is pressed at the fourth prompt
- **THEN** point lands on the second command's input start
