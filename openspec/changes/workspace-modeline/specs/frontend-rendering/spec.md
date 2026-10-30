## ADDED Requirements

### Requirement: The workspace modeline is drawn above every window
The frontend SHALL draw one workspace modeline row across the frame above the window tree, one line high in the editor's font, showing the project name bold, ` › ` dim and the workspace name, on the inactive mode line background. The windows SHALL be laid out below it, so it never covers buffer text, in windowed and fullscreen frames alike.

#### Scenario: One bar above the splits
- **WHEN** the frame shows two side-by-side windows in workspace `main` of project `home`
- **THEN** one row reading `home › main` is drawn above both windows and the windows start below it
