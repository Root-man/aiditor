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
A terminal buffer's text SHALL be empty outside copy mode and the buffer read-only, so text commands run in it by name without panicking: motion, `mark-whole-buffer` and line search find nothing to move over, and edits fail with `Buffer is read-only` as in any read-only buffer.

#### Scenario: Rope commands
- **WHEN** a terminal buffer is current and `forward-char`, `next-line`, `end-of-buffer` and `mark-whole-buffer` run in it with `M-x`
- **THEN** none panics and point stays at 0

#### Scenario: Edits
- **WHEN** `newline` runs with `M-x` in a terminal buffer
- **THEN** the echo area says `Buffer is read-only` and its grid is unchanged

### Requirement: vterm opens a terminal buffer
`vterm` (through `M-x`) SHALL spawn the user's login shell in a new terminal buffer of the current workspace and show it in the selected window. The buffer SHALL be named `vterm<n><workspace>`, `n` the smallest number from 1 no buffer of that name has. The shell's working directory SHALL be the current buffer's file's directory, a terminal buffer's directory, or the primary root of the current workspace's project when the buffer has neither.

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

#### Scenario: Working directory from a terminal
- **WHEN** a terminal's shell reported `/tmp` through OSC 7 and `M-x vterm` runs from its buffer
- **THEN** the new shell is spawned in `/tmp`

#### Scenario: Working directory from a terminal without OSC 7
- **WHEN** a terminal spawned in `/r/app` reported no directory and `M-x vterm` runs from its buffer
- **THEN** the new shell is spawned in `/r/app`

### Requirement: The terminal is sized to its smallest window
A live terminal's grid and PTY SHALL be as many rows as the text area (the window less its top padding and mode line) and as many columns as fit between its 12 px side insets in the narrowest and shortest window of the current workspace showing it, taking rows and columns independently, as Emacs's `window-adjust-process-window-size-smallest`. When that size changes, the core SHALL resize the grid and send one `TerminalOp::Resize`. A terminal no window shows, or one before the frame size is known, SHALL keep its size; a new one starts at 24x80 until the frame size is known.

#### Scenario: Spawned at the window's size
- **WHEN** the window's text area is 80 cells by 24 lines and `M-x vterm` runs in it
- **THEN** the shell is spawned with 24 rows and 80 columns

#### Scenario: Splitting shrinks it
- **WHEN** a terminal is shown in the only window and `C-x 2` splits it
- **THEN** the core resizes the PTY to the smaller of the two windows' text areas

#### Scenario: Resizing the frame
- **WHEN** the frame showing a terminal gets smaller
- **THEN** the core resizes the PTY to the new size, once

### Requirement: The buffer follows the shell
When the shell exits, its buffer SHALL be killed. When the spawn fails, the buffer SHALL be killed and the reason shown in the echo area. In both cases the core SHALL send the terminal one kill, so the service releases its PTY and input thread at once rather than at quit. When a terminal buffer is killed by any path, its shell SHALL be killed.

#### Scenario: exit
- **WHEN** the terminal service reports a terminal's shell exited
- **THEN** its buffer is killed and the core sends exactly one `TerminalOp::Kill` for that terminal, and no other op

#### Scenario: Spawn failure
- **WHEN** the terminal service reports a spawn failed with `/x: not a directory`
- **THEN** the buffer is killed, the echo area shows `/x: not a directory`, and the core sends exactly one `TerminalOp::Kill` for that terminal

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
Outside copy mode, a window showing a terminal buffer SHALL display the grid's screen rows, top to bottom, not its scrollback, and its mode line as any window. Each row SHALL be runs of cells, each run at its grid column with its cell count, text, foreground, background (none for the default background) and bold, italic and underline flags; blank default cells need no run.

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
A wide char SHALL take two columns, its run counting two cells, so the chars after it sit at the same columns as in the rows below. An emoji such as `🦀` is a wide char.

#### Scenario: Japanese text
- **WHEN** `日本x\r\nabcde` is fed
- **THEN** `x` is at column 4 on the first row, the same column as `e` on the second

#### Scenario: Emoji
- **WHEN** `🦀x\r\nabc` is fed
- **THEN** `🦀` is a run of two cells at column 0 and `x` is at column 2, the same column as `c` on the second row

### Requirement: Char mode sends keys to the shell
While a terminal buffer's shell is live, it isn't in copy mode and no key sequence is pending, every key except `C-x`, `C-c`, `M-x`, keys with super held and, on Linux, `C-S-v` and the zoom keys `C-=`, `C-+`, `C--` and `C-0` SHALL be written to the shell, encoded as xterm does, instead of running a command. Those and the keys that follow a prefix SHALL go through the keymaps as in any buffer. A key with no encoding SHALL be dropped.

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

#### Scenario: Zoom keys in a terminal
- **WHEN** the platform's zoom key is pressed in a terminal in char mode
- **THEN** nothing is written to the shell and the font size changes

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
Text from the platform's text input, including input-method commits, SHALL be written to a live terminal's shell as UTF-8 when it isn't in copy mode and no key sequence is pending; a range the input method asks to replace SHALL be ignored.

#### Scenario: Non-ASCII text
- **WHEN** `é` arrives as typed text in a terminal
- **THEN** the shell receives its UTF-8 bytes

### Requirement: The paste key pastes into the shell
The paste key, `s-v` on macOS and `C-S-v` on Linux, in a live terminal buffer SHALL write the kill at the yank pointer, the newest unless `M-y` moved it, to its shell, after taking in clipboard text from another app as `yank` does, instead of inserting it. With an empty kill ring and clipboard it SHALL echo `Kill ring is empty` and write nothing.

#### Scenario: Paste the clipboard
- **WHEN** the clipboard holds `ls` from another app and the paste key is pressed in a terminal
- **THEN** the shell receives `ls`

#### Scenario: C-v still reaches the shell
- **WHEN** `C-v` is pressed in a terminal
- **THEN** the shell receives `\x16`

### Requirement: Paste is bracketed when the program asks
While the program has bracketed paste on (mode 2004), pasted text SHALL be sent between `\x1b[200~` and `\x1b[201~` with every ESC in it removed; otherwise it SHALL be sent as is.

#### Scenario: Bracketed
- **WHEN** the program sent `\x1b[?2004h` and `a\nb\x1b[201~c` is pasted
- **THEN** the shell receives `\x1b[200~a\nb[201~c\x1b[201~`

#### Scenario: Not bracketed
- **WHEN** the program has not turned on bracketed paste and `a\nb` is pasted
- **THEN** the shell receives `a\nb`

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
Copy mode's text SHALL carry each cell's resolved foreground and background colours, bold, italic and underline, as the live screen draws them. Leaving copy mode SHALL drop them with the text.

#### Scenario: Coloured output
- **WHEN** `\x1b[31mred\x1b[0m` is fed into a terminal and copy mode is entered
- **THEN** the window draws `red` in the theme's ANSI red

#### Scenario: Font attributes
- **WHEN** a bold, an italic and an underlined char are fed into a terminal and copy mode is entered
- **THEN** each char of the text is drawn bold, italic or underlined as on the live screen

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
A terminal SHALL take OSC 7's `file://host/path` or `kitty-shell-cwd://host/path` as its current directory, percent-decoding the path, when the host is empty, `localhost` or this machine's hostname and the decoded path is absolute. Any other OSC 7 SHALL leave the directory unchanged, since program output is untrusted. A terminal starts with the directory its shell is spawned in.

#### Scenario: Local host
- **WHEN** `\e]7;file://localhost/tmp/a%20b\a` is fed, then `\e]7;file:///home\e\\`
- **THEN** the directory is first `/tmp/a b`, then `/home`

#### Scenario: This machine
- **WHEN** OSC 7 names this machine's hostname with path `/srv`
- **THEN** the directory is `/srv`

#### Scenario: Remote, relative or garbage
- **WHEN** after `/home`, OSC 7 is fed with `file://elsewhere.example/etc`, `file://localhost` (no path), `file:tmp` (relative), `http://localhost/x` or `file:///bad%zz`
- **THEN** the directory stays `/home`

#### Scenario: Before any OSC 7
- **WHEN** `M-x vterm` spawns a shell in `/r/app` and no OSC 7 has been fed
- **THEN** the terminal's directory is `/r/app`

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
