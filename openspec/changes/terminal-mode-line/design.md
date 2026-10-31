# Design

## Context

`Terminal::feed` records OSC 133 marks with absolute positions and stops recording them once the scrollback is full, and on the alternate screen. `display::mode_line` builds the terminal's segments with every other non-file buffer.

## Goals / Non-Goals

**Goals:** the mode line of frame 9, correct for as long as a terminal lives.

**Non-Goals:** integration for other shells; a history of commands; changing how marks are stored.

## Decisions

**The shell sends the command text.** Reading it from the grid between the `B` and `C` marks needs line numbers that stop being true at a full scrollback, which 10,000 lines of output reach quickly, and it would read `PS2` prompts into a multi-line command. `$1` of `preexec` is the line as typed, so `133;C;cmdline_url=` costs one shell line and a decoder that OSC 7 already has. `cmdline_url` is kitty's and Ghostty's name, so a shell set up for those works too.

**The state is separate from the marks.** `running` and `exit` update on every mark, on either screen and past a full scrollback. A program that dies on the alternate screen must not leave `◉` up, since zsh's `D` then arrives there. Marks carry positions and keep their rules.

**Transitions.** `C` sets `running` to its command and clears `exit`, as a stale failure next to a new command reads as that command's. `D` clears `running` and sets `exit` to a non-zero code. `A` clears `running`, for shells that send no `D`. An empty `RET` sends no marks, so `exit` stays until the next command.

**A separate function for the mode line**, as `file_mode_line`: terminals share nothing with the `%%` group any more.

**Control chars in a command show as spaces.** A multi-line command is one line in the bar, and the text comes from program output, so nothing in it may reach the shaper as a newline.

## Risks / Trade-offs

- Output is untrusted, so `cat`ing a crafted file can fake a command or an exit in the mode line. It is display only, and marks already share this.
- Output waits while in copy mode, so `◉` and `exit` hold still there until copy mode ends.
- A long directory is cut from its end by the mode line's usual rule, unlike a file's directory.
