# line-numbers Specification

## Purpose
Shows line numbers in a gutter beside the text of file buffers, `*scratch*` and `*Messages*`, as Emacs's `display-line-numbers-mode`, so a line can be found and named at a glance.

## Requirements

### Requirement: The setting picks the default
The `line_numbers` setting SHALL be `off`, `absolute` or `relative`, bundled as `absolute`, and SHALL apply to every file buffer, `*scratch*` and `*Messages*` that has not been toggled, a reloaded value included. Any other value SHALL be rejected as an invalid setting. All other buffers SHALL show no gutter.

#### Scenario: A file buffer by default
- **WHEN** a file is opened with no user settings
- **THEN** its window has a gutter of absolute numbers

#### Scenario: Turned off
- **WHEN** `line_numbers = "off"` is loaded
- **THEN** file windows have no gutter

#### Scenario: Scratch and Messages
- **WHEN** `*scratch*` or `*Messages*` is shown with no user settings
- **THEN** its window has a gutter of absolute numbers

#### Scenario: Another buffer
- **WHEN** a buffer that visits no file, made with `C-x b`, is shown
- **THEN** its window has no gutter

#### Scenario: A bad value
- **WHEN** the user file sets `line_numbers = "sometimes"`
- **THEN** an error is shown and the previous settings stay in effect

### Requirement: The command toggles the gutter per buffer
`display-line-numbers-mode` SHALL hide a shown gutter and show a hidden one for the current buffer only, if it is one of those, echoing `Display-Line-Numbers mode enabled in current buffer` or `disabled`. Shown over `line_numbers = "off"` the numbers SHALL be absolute; otherwise in the setting's style. In any other buffer it SHALL fail with `No line numbers in this buffer`.

#### Scenario: Toggling one buffer
- **WHEN** two files are open and the command runs in the first
- **THEN** the first has no gutter and the second keeps its own

#### Scenario: Toggling on over the setting
- **WHEN** `line_numbers` is `off` and the command runs in a file buffer
- **THEN** that buffer shows absolute numbers

#### Scenario: Toggling back keeps the style
- **WHEN** `line_numbers` is `relative` and the command runs twice
- **THEN** the buffer shows relative numbers

#### Scenario: A buffer that takes no numbers
- **WHEN** the command runs in a terminal
- **THEN** it fails with `No line numbers in this buffer`

### Requirement: Numbers name each line
In `absolute` style each line SHALL show its number from 1. In `relative` style each line SHALL show its distance in buffer lines from point's line, and point's line its absolute number. A window that shows point's line SHALL draw that number in `line-number-current` and the others in `line-number`, right-aligned in the gutter.

#### Scenario: Absolute
- **WHEN** point is on the second of three lines
- **THEN** the gutter reads 1, 2, 3 with 2 in `line-number-current`

#### Scenario: Relative
- **WHEN** `line_numbers` is `relative` and point is on the third of five lines
- **THEN** the gutter reads 2, 1, 3, 1, 2 with 3 in `line-number-current`

### Requirement: Continuation rows show a marker
Every visual row of a wrapped line but its first SHALL show `↪` in `line-number` in place of a number, also when that row is the window's first or holds point.

#### Scenario: A wrapped line
- **WHEN** a line wraps into two rows
- **THEN** its first row shows its number and its second `↪`

#### Scenario: A window starting inside a line
- **WHEN** the window's first row is the second row of a line
- **THEN** that row shows `↪`

### Requirement: The gutter takes its width from the text
The gutter SHALL be 10px, then the digits of the buffer's line count in cells, then 12px wide, and text SHALL start after it. Lines SHALL wrap and scroll sideways in the columns left, so the gutter never overlaps text. A window with no column left SHALL show no gutter.

#### Scenario: Growing with the line count
- **WHEN** a nine-line buffer in 8 px cells gets a tenth line
- **THEN** the gutter grows from 30 to 38 px

#### Scenario: Wrapping in what is left
- **WHEN** a nine-line file is shown in a window 110 px wide in 8 px cells
- **THEN** rows wrap in 10 columns less the marker's

#### Scenario: Drawn beside the text
- **WHEN** a file window is drawn
- **THEN** each number's right edge is 12 px before the text's left edge, and the cursor stays over its char

#### Scenario: Too narrow
- **WHEN** a window is no wider than the gutter plus one cell
- **THEN** it shows no gutter
