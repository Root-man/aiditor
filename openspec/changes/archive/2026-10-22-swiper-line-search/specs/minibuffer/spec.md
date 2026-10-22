# Spec Delta

## ADDED Requirements

### Requirement: A command can filter its own candidates
A command's spec SHALL be able to filter its candidates itself in place of the fuzzy matcher, given the text and every candidate, returning those that match in the order it chooses. When the text changes, the selected candidate SHALL stay selected if it still matches, else the next match after it in the given order SHALL be, else the last match. `RET` with nothing matching SHALL show `Match required` and keep the minibuffer open.

#### Scenario: Matches keep the filter's order
- **WHEN** a filtering prompt lists `b1`, `a`, `b2` and `b` is typed
- **THEN** the list shows `b1` then `b2`

#### Scenario: A match is required
- **WHEN** at a filtering prompt the text matches nothing and `RET` is typed
- **THEN** the echo area shows `Match required`, the minibuffer stays open and the command does not run

### Requirement: A command can preselect a candidate
A command's spec SHALL be able to name, given the args so far, which candidate is selected when its list opens.

#### Scenario: The preselected candidate
- **WHEN** a prompt over `a`, `b`, `c` preselects the second
- **THEN** `b` is selected when the list opens, and the count reads `2/3`

### Requirement: A preview can show a position in the window's buffer
A preview SHALL be able to name a position in the buffer of the window the minibuffer was opened from. That window SHALL show its own buffer with point there, keeping its scroll from one such preview to the next and centering point's line only when it is out of view. Closing the minibuffer SHALL restore the window as for any preview.

#### Scenario: A position preview keeps the scroll while in view
- **WHEN** one position preview scrolls the window to center line 100, and the next names line 102
- **THEN** the window shows line 102 without scrolling

### Requirement: A command's keymap applies in its minibuffer
A minibuffer SHALL have the name of the command it reads for as a minor mode, so a keymap named after the command binds keys at its prompts.

#### Scenario: A command's key at its prompt
- **WHEN** the keymap `swiper` binds `C-s` and swiper's prompt is open
- **THEN** `C-s` runs that binding, not the global one

### Requirement: A filtering prompt's history keeps the typed text
A filtering prompt SHALL add the typed text to its history instead of the selected candidate, both when it returns with `RET` and when it is aborted with `C-g`, following the history's rules for empty and repeated entries.

#### Scenario: The typed text is kept
- **WHEN** at a filtering prompt `b` is typed, `b1` is selected and `RET` is typed, and the prompt is opened again and `M-p` typed
- **THEN** the text is `b`
