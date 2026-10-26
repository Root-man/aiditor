## ADDED Requirements

### Requirement: Line motion and scrolling skip folded lines
A buffer line that starts inside invisible text (a fold) SHALL count as part of the line the fold starts on: `C-n` from that line SHALL go to the first line after the fold's end, `C-p` onto a folded line SHALL go to the fold's first line, and scrolling SHALL count a fold's lines as none, so a window keeps a fold's first line in view above point.

#### Scenario: Moving past a folded tool call
- **WHEN** point is on a folded tool call's header, whose content has 50 lines, and `C-n` is typed, then `C-p`
- **THEN** point goes to the blank line after the call, then back to the header

#### Scenario: Scrolling over a fold
- **WHEN** a window of 5 rows shows a session whose last block is a folded tool call with 50 lines of content, with point on the prompt line
- **THEN** the call's header is shown above the prompt line
