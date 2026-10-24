## ADDED Requirements

### Requirement: Copy mode marks a command's output
In copy mode, `C-c C-r` (`vterm-mark-output`) SHALL set an active region over the output of the command at point, with point at the output's start and the mark at its end. The command at point SHALL be the last command whose start (its `A` mark, else its `B`) is at or before point. Its output SHALL run from its `C` mark to its `D` mark, else to the next command's start, else to the end of copy mode's text, without the newline that ends its last line. With no command at point, or one without a `C` mark, it SHALL echo `No command at point` and leave point and the region.

#### Scenario: Point on a command's output
- **WHEN** copy mode is entered after `echo 2` printed `2` under OSC 133 marks, point is on that `2` and `C-c C-r RET` is pressed
- **THEN** the newest kill is `2`

#### Scenario: Point on a command's line
- **WHEN** point is on the `echo 2` command line and `C-c C-r` is pressed
- **THEN** the region covers `2`

#### Scenario: The last command has no end
- **WHEN** a command printed output after its `C` mark and no `D` or prompt followed, and `C-c C-r` is pressed on it
- **THEN** the region runs from its output's start to the end of copy mode's text

#### Scenario: No marks
- **WHEN** `C-c C-r` is pressed in copy mode of a terminal with no marks
- **THEN** `No command at point` is echoed and point stays
