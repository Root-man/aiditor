# terminal-buffers Specification

## Purpose
Holds a terminal emulator's grid in a buffer, fed by the terminal's output, with the emulator's replies sent back to the terminal.

## Requirements

### Requirement: A terminal buffer holds the emulator's screen
A terminal buffer SHALL hold a VT emulator grid of the rows and columns it was created with. The terminal's output, fed in order, SHALL leave the grid's screen as a real xterm-compatible terminal would show it, cursor moves, erases, line wraps and scrolling included. Lines that scroll off the top SHALL go to the scrollback, not be lost.

#### Scenario: Plain lines
- **WHEN** `one\r\ntwo\r\n` is fed into a 4x10 terminal buffer
- **THEN** its screen reads `one`, `two` and two empty lines

#### Scenario: Cursor moves and clears
- **WHEN** `hello\x1b[1;1Hj` is fed, then `\x1b[2J\x1b[Hnew`
- **THEN** the screen first reads `jello`, then only `new`

#### Scenario: Scrolling
- **WHEN** five lines `1` to `5` are fed into a 3-row terminal buffer
- **THEN** the screen reads `3`, `4` and `5`, and `1` and `2` are in the scrollback

#### Scenario: Output split across batches
- **WHEN** an escape sequence is split between two output batches
- **THEN** the screen is the same as if it arrived in one

### Requirement: The emulator's replies go back to the terminal
Replies the emulator writes back to the program, such as cursor position and device attribute reports, SHALL be sent as input to that terminal, in order, after the output batch that asked for them. Title changes, the bell and clipboard requests SHALL be ignored.

#### Scenario: Cursor position report
- **WHEN** `ab\x1b[6n` is fed into a terminal buffer
- **THEN** the core asks the terminal service to write `\x1b[1;3R` to that terminal

#### Scenario: Device attributes
- **WHEN** `\x1b[c` is fed into a terminal buffer
- **THEN** the core writes a primary device attributes reply to that terminal

#### Scenario: Ignored requests
- **WHEN** a title change, a bell and an OSC 52 clipboard store are fed into a terminal buffer
- **THEN** nothing is written back and the clipboard is untouched

### Requirement: Output for an unknown terminal is ignored
Output for a terminal id the core has no live terminal buffer for SHALL change nothing and send nothing.

#### Scenario: Unknown id
- **WHEN** output arrives for a terminal id the core never created
- **THEN** no buffer changes and no effect is emitted

### Requirement: A terminal buffer is read-only text
A terminal buffer's text SHALL be empty and the buffer read-only, so text commands run in it by name without panicking: motion, `mark-whole-buffer` and line search find nothing to move over, and edits fail with `Buffer is read-only` as in any read-only buffer.

#### Scenario: Rope commands
- **WHEN** a terminal buffer is current and `forward-char`, `next-line`, `end-of-buffer` and `mark-whole-buffer` run in it with `M-x`
- **THEN** none panics and point stays at 0

#### Scenario: Edits
- **WHEN** `newline` runs with `M-x` in a terminal buffer
- **THEN** the echo area says `Buffer is read-only` and its grid is unchanged

### Requirement: vterm opens a terminal buffer
`vterm` (through `M-x`) SHALL spawn the user's login shell in a new terminal buffer of the current workspace and show it in the selected window. The buffer SHALL be named `vterm<n><workspace>`, `n` the smallest number from 1 no buffer of that name has. The shell's working directory SHALL be the current buffer's file's directory, or the primary root of the current workspace's project when the buffer visits no file.

#### Scenario: Names count within a workspace
- **WHEN** `M-x vterm` runs twice in workspace `work`, then once in workspace `fix`
- **THEN** the buffers are `vterm1<work>`, `vterm2<work>` and `vterm1<fix>`

#### Scenario: A freed number is reused
- **WHEN** `vterm1<work>` and `vterm2<work>` exist, `vterm1<work>` is killed and `M-x vterm` runs in `work`
- **THEN** the new buffer is `vterm1<work>`

#### Scenario: Working directory from the file
- **WHEN** `M-x vterm` runs from a buffer visiting `/r/app/src/main.rs`
- **THEN** the shell is spawned in `/r/app/src`

#### Scenario: Working directory from the project
- **WHEN** `M-x vterm` runs from `*scratch*` in a workspace whose project's primary root is `/r/app`
- **THEN** the shell is spawned in `/r/app`

### Requirement: The terminal is sized to its smallest window
A live terminal's grid and PTY SHALL be as many rows as the text area (the window less its mode line) and as many columns as fit in the narrowest and shortest window of the current workspace showing it, taking rows and columns independently, as Emacs's `window-adjust-process-window-size-smallest`. When that size changes, the core SHALL resize the grid and send one `TerminalOp::Resize`. A terminal no window shows, or one before the frame size is known, SHALL keep its size; a new one starts at 24x80 until the frame size is known.

#### Scenario: Spawned at the window's size
- **WHEN** the frame is 80 cells by 25 lines and `M-x vterm` runs in one window
- **THEN** the shell is spawned with 24 rows and 80 columns

#### Scenario: Splitting shrinks it
- **WHEN** a terminal is shown in the only window and `C-x 2` splits it
- **THEN** the core resizes the PTY to the smaller of the two windows' text areas

#### Scenario: Resizing the frame
- **WHEN** the frame showing a terminal gets smaller
- **THEN** the core resizes the PTY to the new size, once

### Requirement: The buffer follows the shell
When the shell exits, its buffer SHALL be killed. When the spawn fails, the buffer SHALL be killed and the reason shown in the echo area. When a terminal buffer is killed by any path, its shell SHALL be killed.

#### Scenario: exit
- **WHEN** the terminal service reports a terminal's shell exited
- **THEN** its buffer is killed and nothing is sent to that terminal

#### Scenario: Spawn failure
- **WHEN** the terminal service reports a spawn failed with `/x: not a directory`
- **THEN** the buffer is killed and the echo area shows `/x: not a directory`

#### Scenario: Killing a workspace
- **WHEN** a workspace holding two terminals is killed
- **THEN** the core sends `TerminalOp::Kill` for both

### Requirement: Killing a live terminal asks first
`kill-buffer` on a terminal whose shell has not exited SHALL ask `Buffer <name> has a running process; kill it? `, completing over `no` then `yes`. Only `yes` SHALL kill the buffer and its shell.

#### Scenario: Yes
- **WHEN** `kill-buffer` names a live terminal and the answer is `yes`
- **THEN** the buffer is killed and `TerminalOp::Kill` is sent for it

#### Scenario: No
- **WHEN** the answer is `no`
- **THEN** the buffer stays and nothing is sent to the terminal

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

### Requirement: Char mode sends keys to the shell
While a terminal buffer's shell is live and no key sequence is pending, every key except `C-x`, `C-c`, `M-x` and keys with super held SHALL be written to the shell, encoded as xterm does, instead of running a command. Those four and the keys that follow a prefix SHALL go through the keymaps as in any buffer. A key with no encoding SHALL be dropped.

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

### Requirement: Keys are encoded as xterm does
Chars SHALL be sent as UTF-8; `C-` with a letter or one of `@[\]^_` as its control byte (shift ignored), `C-SPC` as `\x00` and `C-?` as `\x7f`; `M-` as an ESC before the key's encoding; `RET` as `\r` (`S-RET` `\n`), `TAB` `\t`, `S-TAB` `\x1b[Z`, `ESC` `\x1b`, `DEL` `\x7f` (`C-DEL` `\x08`). Unmodified arrows, Home and End SHALL send `\x1b[X` in normal cursor mode and `\x1bOX` in application cursor mode, as the program set with DECCKM; with modifiers, `\x1b[1;<m>X`, where `<m>` is 1 plus 1 for shift, 2 for meta and 4 for ctrl. PageUp, PageDown and Delete SHALL send `\x1b[5~`, `\x1b[6~` and `\x1b[3~`; F1–F4 `\x1bOP`–`\x1bOS`; F5–F12 `\x1b[<n>~` with n 15, 17–21, 23, 24; modified, `\x1b[<n>;<m>~` and `\x1b[1;<m>P`–`S`.

#### Scenario: Cursor modes
- **WHEN** `<up>` is pressed before and after the program sends `\x1b[?1h`
- **THEN** the shell receives `\x1b[A`, then `\x1bOA`

#### Scenario: Modified arrows
- **WHEN** `C-<left>` is pressed in either cursor mode
- **THEN** the shell receives `\x1b[1;5D`

### Requirement: C-c C-c interrupts
`C-c C-c` in a terminal buffer SHALL write `\x03` to its shell.

#### Scenario: Interrupt
- **WHEN** `C-c C-c` is pressed in a terminal
- **THEN** the shell receives `\x03`

### Requirement: Typed text goes to the shell
Text from the platform's text input, including input-method commits, SHALL be written to a live terminal's shell as UTF-8 when no key sequence is pending; a range the input method asks to replace SHALL be ignored.

#### Scenario: Non-ASCII text
- **WHEN** `é` arrives as typed text in a terminal
- **THEN** the shell receives its UTF-8 bytes
