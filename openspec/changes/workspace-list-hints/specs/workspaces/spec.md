# Spec Delta

## ADDED Requirements

### Requirement: Workspace list keys live in the echo line
The workspace list SHALL start with its first project row, without a header or leading blank row. While the selected window shows a workspace list and no message or minibuffer is up, the echo line SHALL show `n/p move   RET switch   TAB fold   q quit`. Keys SHALL use bold text in the `workspace-current` foreground and action labels SHALL use `mode-line-dim`, as frames 13–13d and 6d. Selecting another window SHALL remove these hints even if the list remains visible elsewhere. A message SHALL replace the hints until the next key; timers SHALL retain the message. An open minibuffer SHALL retain its ordinary prompt. Hints SHALL wrap between complete actions into a second row when they do not fit, and the windows SHALL yield the extra echo height.

#### Scenario: Context follows the selected window
- **GIVEN** a split with the workspace list selected and an ordinary buffer in the other window
- **WHEN** the list opens and selection then moves to the other window and back
- **THEN** the first buffer row is a project, the hints are shown for the selected list, disappear for the ordinary buffer and return for the list

#### Scenario: A message replaces hints for one key
- **GIVEN** point is on an informational buffer row in the workspace list
- **WHEN** RET is pressed, a timer fires, and then n is pressed
- **THEN** the echo line shows `Select a project or workspace row` through the timer, and returns to the hints after n

#### Scenario: A prompt has precedence
- **GIVEN** the selected window shows the workspace list hints
- **WHEN** M-x opens and then its prompt is accepted to reopen the list
- **THEN** the ordinary minibuffer prompt replaces the hints while open, and the hints return when the list is selected again

#### Scenario: Hints wrap and windows yield height
- **GIVEN** a frame wide enough for all four action hints on one row
- **WHEN** the frame narrows enough to fit the hints on two rows and later widens again
- **THEN** complete hints wrap to the second row with their key styles retained, every window yields one extra echo row, and widening restores the original echo height
