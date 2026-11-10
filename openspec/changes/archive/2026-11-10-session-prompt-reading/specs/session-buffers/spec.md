# Spec Delta

## MODIFIED Requirements

### Requirement: Read-only transcript, editable prompt area
Text before the prompt area SHALL be read-only: an edit that inserts before the prompt area's start or deletes any of the text before it SHALL be refused with `Text is read-only` and change nothing. The prompt area, from after the `> ` prompt line to the end of the buffer, SHALL edit like any buffer, undo included. Output SHALL be inserted above the prompt line, moving the prompt area down and keeping its text and undo. A command that leaves point on the prompt line's `> ` SHALL leave it at the prompt area's start instead. A char that would insert itself, typed with point in the transcript, SHALL insert at the window's prompt point (where point was last in the prompt area, else the buffer's end) and leave point after it, and the transcript SHALL keep its rows. Keys the session binds to other commands, such as `TAB` and the permission and question keys, SHALL keep their meaning.

#### Scenario: Point skips the prompt marker
- **WHEN** `C-a` is typed on the prompt line, or `C-b` at the prompt area's start
- **THEN** point is at the prompt area's start, after `> `

#### Scenario: Editing the transcript is refused
- **WHEN** point is in the transcript and `C-d` is typed
- **THEN** the echo area shows `Text is read-only` and the text is unchanged

#### Scenario: Typing in the transcript goes to the prompt
- **WHEN** point is in the transcript, scrolled back, and `a` is typed
- **THEN** `a` is inserted at the prompt point, point is after it and the transcript rows are unchanged

#### Scenario: TAB still folds
- **WHEN** point is on a tool group line in the transcript and `TAB` is typed
- **THEN** the group opens and nothing is inserted in the prompt area

#### Scenario: The prompt area edits
- **WHEN** `ab DEL c` is typed in the prompt area
- **THEN** the prompt area holds `ac`, and `C-/` undoes the last edit

#### Scenario: Output keeps the prompt area
- **WHEN** a message chunk arrives while the prompt area holds `draft`
- **THEN** the chunk appears above the prompt line and the prompt area still holds `draft`

### Requirement: The prompt area is docked at the window's bottom
A window showing a session buffer SHALL draw the prompt area, from the live prompt line to the end of the buffer, in a pane pinned to the bottom of its text area, and the transcript in the rows above it. The pane SHALL have as many rows as the prompt area takes, wrapped rows included, up to a third of the window's text rows and at least one; past that the pane SHALL scroll to keep point visible. With point in the transcript the transcript rows SHALL scroll to keep point visible, as a window does. With point in the prompt area the transcript rows SHALL stay where they were, never past the transcript's end, and SHALL follow new output only while they show the transcript's end; a window starts showing the end, and sending a prompt SHALL show the end again. With point in the prompt area, `C-v` and `M-v` SHALL page the transcript rows and `C-l` SHALL recenter them on the transcript point, without moving point; paging SHALL move the transcript point into the rows shown, and paging past either end SHALL echo `End of buffer` or `Beginning of buffer`. Transcript text SHALL never show in the pane, nor the prompt area in the transcript rows. Windows showing other buffers SHALL be unchanged.

#### Scenario: A short transcript
- **WHEN** a session with a three-line transcript is shown in a window of 30 text rows
- **THEN** the prompt line is drawn in the window's last text row, under the pane's rule, and the transcript in the first rows

#### Scenario: Output arrives while typing
- **WHEN** point is in the prompt area, the transcript rows show its end, and more transcript lines arrive than the transcript rows hold
- **THEN** the transcript rows show the transcript's last lines and the prompt pane is unchanged

#### Scenario: Output arrives while typing scrolled back
- **WHEN** point is in the prompt area, the transcript rows are scrolled back, and output arrives and chars are typed
- **THEN** the transcript rows are unchanged

#### Scenario: Reading the transcript
- **WHEN** point moves to the transcript's first line with `M-<`
- **THEN** the transcript rows show the transcript from its first line and the prompt pane still shows the prompt area

#### Scenario: Paging from the prompt area
- **WHEN** point is in the prompt area and `M-v`, `C-v` or `C-l` is typed
- **THEN** the transcript rows move and point is unchanged

#### Scenario: Sending shows the end
- **WHEN** the transcript rows are scrolled back and a prompt is sent
- **THEN** the transcript rows show the transcript's end

#### Scenario: A long prompt
- **WHEN** the prompt area holds 20 lines in a window of 30 text rows
- **THEN** the pane has 10 rows, showing point's line

## ADDED Requirements

### Requirement: C-t moves between the prompt and the transcript
Each window showing a session SHALL remember a transcript point and a prompt point: where point was last in each part. `agent-toggle-prompt`, bound to `C-t` in `agent-session-mode`, SHALL move point from the prompt area to the transcript point, first moved into the transcript rows shown (the last shown line when it has none), and from the transcript to the prompt point, else the buffer's end. Neither way SHALL change the transcript rows. Outside a session buffer it SHALL echo `Not in an agent session buffer`.

#### Scenario: There and back
- **WHEN** point is on a transcript line shown mid-window, `M->` is typed, then `C-t`, then `C-t`
- **THEN** the first `C-t` puts point back on that transcript position and the second at the prompt point, and the transcript rows are the same throughout

#### Scenario: A transcript point out of view
- **WHEN** the transcript rows were paged away from the transcript point and `C-t` is typed in the prompt area
- **THEN** point lands within the transcript rows shown and they do not move
