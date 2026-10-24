## MODIFIED Requirements

### Requirement: C-w kills the region
`C-w` and the cut key, `s-x` on macOS and `C-S-x` on Linux, SHALL run `kill-region`, which kills the text between point and the mark, active or not. With no mark it SHALL fail with `The mark is not set now, so there is no region`.

#### Scenario: Kill the region
- **WHEN** in `abcd` the mark is at 1 and point at 3, and `C-w` is pressed
- **THEN** the text is `ad`, point is 1 and the region is inactive

### Requirement: M-w copies the region
`M-w` and the copy key, `s-c` on macOS and `C-S-c` on Linux, SHALL run `kill-ring-save`, which saves the text between point and the mark on the kill ring without changing the buffer, and deactivates the mark.

#### Scenario: Copy and yank
- **WHEN** in `ab` the mark is at 0 and point at 2, and `M-w C-y` is pressed
- **THEN** the text is `abab` and the region is inactive

### Requirement: C-y yanks the newest kill
`C-y` and the paste key, `s-v` on macOS and `C-S-v` on Linux, SHALL run `yank`, which sets the mark at point without activating it, inserts the entry at the yank pointer and leaves point after it. A plain `C-u` SHALL leave point before the text and the mark after it. A number N SHALL yank the Nth entry from the pointer, and a lone minus the entry before it. With an empty ring it SHALL fail with `Kill ring is empty`.

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
