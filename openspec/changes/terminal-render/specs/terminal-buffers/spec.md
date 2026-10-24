# Spec Delta

## ADDED Requirements

### Requirement: A terminal window displays the live screen
A window showing a terminal buffer SHALL display the grid's screen rows, top to bottom, not its scrollback, and its mode line as any window. Each row SHALL be runs of cells, each run at its grid column with its cell count, text, foreground, background (none for the default background) and bold, italic and underline flags; blank default cells need no run.

#### Scenario: Text at its columns
- **WHEN** `ab\x1b[1;6Hc` is fed into a 3x10 terminal shown in a window
- **THEN** the display's first row has `ab` at column 0 and `c` at column 5, in the theme's foreground

#### Scenario: Scrolled lines are not shown
- **WHEN** five lines `1` to `5` are fed into a 3-row terminal
- **THEN** its window displays `3`, `4` and `5`

### Requirement: Terminal colours resolve against the theme
Named colours 0 to 15 SHALL be the theme's ANSI palette, unless the program set that index with OSC 4. Indices 16 to 231 SHALL be the xterm 6x6x6 cube, 232 to 255 the xterm gray ramp, and truecolor its exact value. Default foreground and background SHALL be the theme's. Inverse SHALL swap foreground and background; dim SHALL draw the foreground at 2/3 of each channel.

#### Scenario: ANSI red from the theme
- **WHEN** `\x1b[31mx` is fed
- **THEN** `x` is drawn in the theme's ANSI colour 1

#### Scenario: 256-colour and truecolor
- **WHEN** `\x1b[38;5;196ma\x1b[38;5;244mb\x1b[38;2;1;2;3mc` is fed
- **THEN** `a` is 0xff0000, `b` is 0x808080 and `c` is 0x010203

#### Scenario: Inverse on defaults
- **WHEN** `\x1b[7mx` is fed
- **THEN** `x` is drawn in the theme's background on the theme's foreground

#### Scenario: Dim
- **WHEN** `\x1b[2;38;2;150;90;30mx` is fed
- **THEN** `x` is drawn in 0x643c14

### Requirement: Terminal attributes reach the display
Bold, italic and underline cells SHALL carry those flags; any underline style counts as underline.

#### Scenario: Bold italic underline
- **WHEN** `\x1b[1ma\x1b[0;3mb\x1b[0;4mc` is fed
- **THEN** `a` is bold, `b` italic and `c` underlined, each in its own run

### Requirement: The terminal cursor follows the program
The display SHALL carry the cursor's row, column, width in cells and shape: block by default, and block, beam or underline as the program last set with DECSCUSR. It SHALL carry no cursor while the program hides it. On a wide char the cursor SHALL be two cells wide.

#### Scenario: DECSCUSR beam
- **WHEN** `ab\x1b[6 q` is fed
- **THEN** the cursor is a beam at row 0, column 2

#### Scenario: Hidden cursor
- **WHEN** `\x1b[?25l` is fed
- **THEN** the display has no cursor, and `\x1b[?25h` brings it back

### Requirement: Wide chars take two cells
A wide char SHALL take two columns, its run counting two cells, so the chars after it sit at the same columns as in the rows below.

#### Scenario: Japanese text
- **WHEN** `日本x\r\nabcde` is fed
- **THEN** `x` is at column 4 on the first row, the same column as `e` on the second
