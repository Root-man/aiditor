## MODIFIED Requirements

### Requirement: Marked text stays out of the buffer
Text an input method marks while composing SHALL be drawn at point inside point's line, underlined, with the cursor after it, and SHALL not be in the buffer, its undo history or keyboard macros. Committing it SHALL insert the committed text as typed text; cancelling it SHALL insert nothing.

#### Scenario: Dead key composes
- **WHEN** the input method marks `´` and then commits `é`
- **THEN** the buffer is unchanged while `´` is marked and holds `é` afterwards

### Requirement: The wheel scrolls the window
A wheel or trackpad scroll SHALL scroll the window under the pointer by whole lines, accumulating smaller movements, without selecting it, and that window's point SHALL stay visible, moving to the start of the nearest visible line as `C-v` does. Scrolling past an end of the buffer SHALL stop there without a message.

#### Scenario: Scroll keeps point visible
- **WHEN** point is on line 0 of a long buffer and the wheel scrolls 5 lines down in a 3-line window
- **THEN** the first line shown is line 5 and point is at its start

#### Scenario: Scroll at the end
- **WHEN** the wheel scrolls past the buffer's last line
- **THEN** the window stops scrolling and no message is shown

#### Scenario: Scroll another window
- **WHEN** the frame is split with `C-x 2` and the wheel scrolls over the lower window
- **THEN** the lower window scrolls, the upper one does not, and the upper one stays selected

### Requirement: A left click moves point
A left click on a window's text SHALL select that window and move its point to the char nearest the click, or to the end of the clicked line or buffer when past it, SHALL deactivate the mark, and SHALL cancel a pending prefix and prefix argument. Typing after a click SHALL start a new undo group.

#### Scenario: Click on a char
- **WHEN** the buffer holds `abc\ndef` and the second line is clicked over `e`
- **THEN** point is before `e`

#### Scenario: Click another window
- **WHEN** the frame is split with `C-x 3` and the right window's text is clicked
- **THEN** the right window is selected and its point is where it was clicked

#### Scenario: Click deactivates the mark
- **WHEN** the region is active and the text is clicked
- **THEN** the region no longer shows

#### Scenario: Click cancels a prefix
- **WHEN** `C-x` is pressed and the text is clicked
- **THEN** no prefix is pending and the next key runs on its own
