# Spec Delta

## ADDED Requirements

### Requirement: Copy mode shows the scrollback as text
`C-c C-t` (`vterm-copy-mode`) in a terminal buffer SHALL enter copy mode: the buffer's text becomes the scrollback then the screen, one line per row with trailing blanks trimmed, rows the terminal soft-wrapped joined into one line, and blank rows below the cursor dropped. Point SHALL be at the terminal's cursor. `C-c C-t` again SHALL leave without copying.

#### Scenario: Scrollback then screen
- **WHEN** `seq 1 5000` output is fed into a terminal and copy mode is entered
- **THEN** the text is the lines `1` to `5000` then the line the cursor is on, and point is at the cursor

#### Scenario: A wrapped line is one line
- **WHEN** a line wider than the terminal is fed and copy mode is entered
- **THEN** that line is a single line of the text

#### Scenario: The alternate screen
- **WHEN** a program on the alternate screen has drawn it and copy mode is entered
- **THEN** the text is the alternate screen's rows only

#### Scenario: Toggling off
- **WHEN** `C-c C-t` is pressed in copy mode
- **THEN** copy mode ends, the kill ring is unchanged and keys go to the shell again

### Requirement: Copy mode keeps the terminal's colours
Copy mode's text SHALL carry each cell's resolved foreground and background colours, as the live screen draws them; bold, italic and underline SHALL NOT carry over. Leaving copy mode SHALL drop them with the text.

#### Scenario: Coloured output
- **WHEN** `\x1b[31mred\x1b[0m` is fed into a terminal and copy mode is entered
- **THEN** the window draws `red` in the theme's ANSI red

#### Scenario: Leaving drops the colours
- **WHEN** copy mode is left
- **THEN** the buffer holds no colours

### Requirement: The mode line shows the terminal's mode
A terminal window's mode line SHALL show `[char]` after the buffer name in char mode, without point's position, and `[copy]` followed by point's line and column in copy mode.

#### Scenario: Char mode
- **WHEN** a terminal buffer `t` is shown in char mode
- **THEN** its mode line reads `%% t [char]`

#### Scenario: Copy mode
- **WHEN** copy mode is entered with point on line 5001, column 1
- **THEN** the mode line reads `%% t [copy]  5001:1`

### Requirement: Copy mode is a read-only buffer
In copy mode, keys and typed text SHALL run commands as in any read-only buffer instead of reaching the shell, and the window SHALL draw the text with point and region, not the live screen. Edits SHALL fail with `Buffer is read-only`.

#### Scenario: Motion, search and copy
- **WHEN** `M-<`, swiper, and `C-SPC` then motion then `M-w` are used in copy mode
- **THEN** they move, search and copy over the text as in a text buffer, and the shell receives nothing

#### Scenario: Inserting fails
- **WHEN** a char is typed in copy mode
- **THEN** the echo area says `Buffer is read-only` and the text is unchanged

### Requirement: Output doesn't change copy mode's text
Output arriving in copy mode SHALL still feed the grid but SHALL NOT change the text or point; it SHALL show once copy mode ends.

#### Scenario: Output during copy mode
- **WHEN** output arrives in copy mode, then copy mode ends
- **THEN** the text and point are unchanged during copy mode, and the live screen shows the output afterwards

### Requirement: RET copies and leaves copy mode
`RET` (`vterm-copy-mode-done`) in copy mode SHALL copy the active region, or the current line without its newline when no region is active, to the kill ring and the clipboard, then leave copy mode. Leaving SHALL empty the text and show the live screen.

#### Scenario: Copying the region
- **WHEN** a region is active in copy mode and `RET` is pressed
- **THEN** the region's text is the newest kill and the clipboard text, and the window shows the live screen in char mode

#### Scenario: Copying the line
- **WHEN** no region is active in copy mode and `RET` is pressed
- **THEN** point's line, without its newline, is the newest kill and the clipboard text

## MODIFIED Requirements

### Requirement: A terminal buffer is read-only text
A terminal buffer's text SHALL be empty outside copy mode and the buffer read-only, so text commands run in it by name without panicking: motion, `mark-whole-buffer` and line search find nothing to move over, and edits fail with `Buffer is read-only` as in any read-only buffer.

#### Scenario: Rope commands
- **WHEN** a terminal buffer is current and `forward-char`, `next-line`, `end-of-buffer` and `mark-whole-buffer` run in it with `M-x`
- **THEN** none panics and point stays at 0

#### Scenario: Edits
- **WHEN** `newline` runs with `M-x` in a terminal buffer
- **THEN** the echo area says `Buffer is read-only` and its grid is unchanged

### Requirement: A terminal window displays the live screen
Outside copy mode, a window showing a terminal buffer SHALL display the grid's screen rows, top to bottom, not its scrollback, and its mode line as any window. Each row SHALL be runs of cells, each run at its grid column with its cell count, text, foreground, background (none for the default background) and bold, italic and underline flags; blank default cells need no run.

#### Scenario: Text at its columns
- **WHEN** `ab\x1b[1;6Hc` is fed into a 3x10 terminal shown in a window
- **THEN** the display's first row has `ab` at column 0 and `c` at column 5, in the theme's foreground

#### Scenario: Scrolled lines are not shown
- **WHEN** five lines `1` to `5` are fed into a 3-row terminal
- **THEN** its window displays `3`, `4` and `5`

### Requirement: Char mode sends keys to the shell
While a terminal buffer's shell is live, it isn't in copy mode and no key sequence is pending, every key except `C-x`, `C-c`, `M-x`, keys with super held and, on Linux, `C-S-v` SHALL be written to the shell, encoded as xterm does, instead of running a command. Those and the keys that follow a prefix SHALL go through the keymaps as in any buffer. A key with no encoding SHALL be dropped.

#### Scenario: Typing a command
- **WHEN** `echo hi` is typed and `RET` pressed in a terminal
- **THEN** the shell receives `echo hi\r`

#### Scenario: Readline keys reach the shell
- **WHEN** `C-a C-e C-r M-b C-g C-u` are pressed in a terminal
- **THEN** the shell receives `\x01\x05\x12\x1bb\x07\x15` and no command runs

#### Scenario: Editor keys still work
- **WHEN** `C-x b`, `M-x` or a `C-c` sequence is pressed in a terminal
- **THEN** its command runs and the shell receives nothing

#### Scenario: Keys go to the minibuffer while it is open
- **WHEN** `M-x` is pressed in a terminal and `vterm` typed
- **THEN** the text goes to the minibuffer, not the shell

### Requirement: Typed text goes to the shell
Text from the platform's text input, including input-method commits, SHALL be written to a live terminal's shell as UTF-8 when it isn't in copy mode and no key sequence is pending; a range the input method asks to replace SHALL be ignored.

#### Scenario: Non-ASCII text
- **WHEN** `é` arrives as typed text in a terminal
- **THEN** the shell receives its UTF-8 bytes
