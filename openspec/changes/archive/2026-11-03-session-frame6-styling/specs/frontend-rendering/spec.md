# Spec Delta

## ADDED Requirements

### Requirement: The session prompt pane is drawn under a rule
A window with a prompt pane SHALL draw the transcript's lines from the top of its text area and the pane's lines at its bottom, just above the mode line. A 1 px rule in the `separator` face's foreground SHALL span the window's width above the pane, with 8 px between the rule and the pane's first row and 8 px between its last row and the mode line. The cursor SHALL be drawn in whichever part holds it.

#### Scenario: The pane at the bottom
- **WHEN** a session window is drawn
- **THEN** its prompt line is drawn 8 px above the mode line, and a 1 px rule spans the window 8 px above the pane's first row

### Requirement: Indented lines are drawn further right
A line reporting an indent of N columns SHALL be drawn with its text, icons, cursor and box runs N cells right of the window's text start, and its background decoration SHALL start there and end at the window's right text inset. Its rule and bar decorations SHALL stay at the window's left edge.

#### Scenario: An indented block line
- **WHEN** a line with a `session-block` background and an indent of 4 is drawn
- **THEN** its background and text start 4 cells right of the text start, its background ends at the right text inset, and its rule stays at the left edge
