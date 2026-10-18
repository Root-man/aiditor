# Spec Delta

## Purpose

The kill ring: the commands that kill text into it and yank it back, how consecutive kills join, and how the ring stays in step with the system clipboard.

## ADDED Requirements

### Requirement: Kills save text on the kill ring
Every kill command SHALL put the text it removes or copies on the front of one kill ring shared by all buffers. The ring SHALL hold at most 60 entries, dropping the oldest. A new entry SHALL reset the yank pointer to it.

#### Scenario: A kill yanks back
- **WHEN** in `foo bar` with point at 0, `M-d M-> C-y` is pressed
- **THEN** the text is ` barfoo`

#### Scenario: The ring keeps 60 entries
- **WHEN** 61 separate kills are made
- **THEN** the ring holds the newest 60

### Requirement: C-k kills to the end of the line
`C-k` SHALL run `kill-line`. Without a prefix argument it SHALL kill from point to the end of the line, or through the newline when only spaces and tabs lie between point and the end of the line. With a prefix argument N it SHALL kill N lines forward, newlines included, or back to the start of the line N lines up when N is 0 or negative. At the end of the buffer it SHALL kill nothing and report `End of buffer`.

#### Scenario: Kill the rest of the line
- **WHEN** in `ab\ncd` with point at 1, `C-k` is pressed
- **THEN** the text is `a\ncd`

#### Scenario: Kill the newline at the end of a line
- **WHEN** in `ab\ncd` with point at 2, `C-k` is pressed
- **THEN** the text is `abcd`

#### Scenario: Kill two lines
- **WHEN** in `a\nb\nc` with point at 0, `C-u 2 C-k` is pressed
- **THEN** the text is `c`

#### Scenario: Kill back to the line start
- **WHEN** in `ab\ncd` with point at 4, `C-u 0 C-k` is pressed
- **THEN** the text is `ab\nd` and point is 3

#### Scenario: At the end of the buffer
- **WHEN** in `ab` with point at 2, `C-k` is pressed
- **THEN** the text is `ab` and the message is `End of buffer`

### Requirement: C-w kills the region
`C-w` and `s-x` SHALL run `kill-region`, which kills the text between point and the mark, active or not. With no mark it SHALL fail with `The mark is not set now, so there is no region`.

#### Scenario: Kill the region
- **WHEN** in `abcd` the mark is at 1 and point at 3, and `C-w` is pressed
- **THEN** the text is `ad`, point is 1 and the region is inactive

### Requirement: M-w copies the region
`M-w` and `s-c` SHALL run `kill-ring-save`, which saves the text between point and the mark on the kill ring without changing the buffer, and deactivates the mark.

#### Scenario: Copy and yank
- **WHEN** in `ab` the mark is at 0 and point at 2, and `M-w C-y` is pressed
- **THEN** the text is `abab` and the region is inactive

### Requirement: Consecutive kills join into one entry
A kill right after another kill SHALL add its text to the ring's newest entry instead of making a new one: after the entry when it killed forward, before it when it killed backward. `M-w` right after a kill SHALL join the same way. Any other command in between SHALL end the run.

#### Scenario: Two C-k yank back as one entry
- **WHEN** in `ab\ncd` with point at 0, `C-k C-k M-> C-y` is pressed
- **THEN** the text is `cdab\n`, and one entry `ab\n` was added to the ring

#### Scenario: Killing backward prepends
- **WHEN** in `foo bar` with point at 4, `M-d M-DEL C-y` is pressed
- **THEN** the text is `foo bar`

#### Scenario: Another command ends the run
- **WHEN** in `ab\ncd` with point at 0, `C-k C-f C-k` is pressed
- **THEN** the ring's two newest entries are `cd` and `ab`

### Requirement: C-y yanks the newest kill
`C-y` and `s-v` SHALL run `yank`, which sets the mark at point without activating it, inserts the entry at the yank pointer and leaves point after it. A plain `C-u` SHALL leave point before the text and the mark after it. A number N SHALL yank the Nth entry from the pointer, and a lone minus the entry before it. With an empty ring it SHALL fail with `Kill ring is empty`.

#### Scenario: Yank
- **WHEN** `ab` is killed and `C-y C-y` is pressed in an empty buffer
- **THEN** the text is `abab` and point is 4

#### Scenario: Yank the second entry
- **WHEN** `a` then `b` are killed separately and `C-u 2 C-y` is pressed
- **THEN** the inserted text is `a`

#### Scenario: Point before the yanked text
- **WHEN** `ab` is killed and `C-u C-y` is pressed in an empty buffer
- **THEN** the text is `ab`, point is 0 and the mark is at 2

#### Scenario: Empty ring
- **WHEN** `C-y` is pressed with nothing killed and nothing on the clipboard
- **THEN** the message is `Kill ring is empty`

### Requirement: M-y replaces the yank with an older kill
`M-y` SHALL run `yank-pop`, which, right after a yank or another `M-y`, replaces the text just yanked with the entry the prefix argument's value (1 without one) further round the ring, wrapping at its end, and moves the yank pointer there. Otherwise it SHALL fail with `Previous command was not a yank`.

#### Scenario: Cycle through the ring
- **WHEN** `a` then `b` are killed separately and `C-y M-y` is pressed in an empty buffer
- **THEN** the text is `a`; one more `M-y` makes it `b`

#### Scenario: Not after a yank
- **WHEN** `M-y` is pressed after `C-f`
- **THEN** the buffer does not change and the message is `Previous command was not a yank`

### Requirement: Kills write the system clipboard
After every kill, the ring's newest entry SHALL be written to the system clipboard as plain text.

#### Scenario: Copy reaches the clipboard
- **WHEN** in `ab` the mark is at 0 and point at 2, and `M-w` is pressed
- **THEN** the clipboard holds `ab`

#### Scenario: Joined kills reach the clipboard whole
- **WHEN** in `ab\ncd` with point at 0, `C-k C-k` is pressed
- **THEN** the clipboard holds `ab\n`

### Requirement: Clipboard text from elsewhere joins the ring
Before a kill or a yank, clipboard text that the editor neither wrote nor already took in SHALL be pushed onto the kill ring as its own entry, so a yank inserts it and a kill keeps it on the ring under the new kill. A kill that pushes such text SHALL start a new entry rather than join the previous kill.

#### Scenario: Yank text copied in another app
- **WHEN** another app puts `xyz` on the clipboard and `C-y` is pressed in an empty buffer
- **THEN** the text is `xyz`

#### Scenario: A kill saves the clipboard first
- **WHEN** another app puts `xyz` on the clipboard and in `ab` with point at 0 `C-k` is pressed
- **THEN** the clipboard holds `ab`, and `C-y M-y` leaves the text `xyz`

#### Scenario: The editor's own text is not pushed again
- **WHEN** `ab` is killed and `C-y` is pressed in an empty buffer
- **THEN** the ring has one entry
