## MODIFIED Requirements

### Requirement: The mode line marks the selected window
Each window's mode line SHALL be drawn in the theme's `mode-line` face for the selected window and in its `mode-line-inactive` face for the others.

#### Scenario: Two windows
- **WHEN** the frame is split with `C-x 2`
- **THEN** the upper mode line uses the `mode-line` face and the lower one `mode-line-inactive`

### Requirement: The workspace modeline is drawn above every window
The frontend SHALL draw one workspace modeline row above the window tree, its background spanning the frame edge to edge through the side gap while its text keeps the 8 px inset, one line high in the editor's font, showing the project name bold, ` › ` in the `separator` face and the workspace name, in the `workspace-bar` face. The windows SHALL be laid out below it, so it never covers buffer text, in windowed and fullscreen frames alike.

#### Scenario: One bar above the splits
- **WHEN** the frame shows two side-by-side windows in workspace `main` of project `home`
- **THEN** one row reading `home › main` is drawn above both windows, from the frame's left edge to its right, and the windows start below it
