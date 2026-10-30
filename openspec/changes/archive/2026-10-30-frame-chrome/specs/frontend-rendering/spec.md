# Spec Delta

## MODIFIED Requirements

### Requirement: Every window is drawn at its rectangle
The frontend SHALL draw every window of the frame display at its rectangle, clipped to it, with its lines from 6 px below its top and its mode line along its bottom. Resizing the OS window SHALL re-lay out and redraw every window.

#### Scenario: A side-by-side split
- **WHEN** `C-x 3` is typed
- **THEN** both windows are drawn side by side, each with its own text and mode line, and a long line in the left window does not draw into the right one

#### Scenario: Resizing the frame
- **WHEN** the OS window is made taller with two stacked windows
- **THEN** both windows grow and show more lines

#### Scenario: Breathing room under the bar
- **WHEN** a window is drawn
- **THEN** its first row starts 6 px below the window's top

### Requirement: The mode line marks the selected window
Each window's mode line SHALL be drawn in the theme's `mode-line` face for the selected window and in its `mode-line-inactive` face for the others. An inactive mode line SHALL have a 1 px rule along its top in the `mode-line` face's background.

#### Scenario: Two windows
- **WHEN** the frame is split with `C-x 2`
- **THEN** the upper mode line uses the `mode-line` face and the lower one `mode-line-inactive`

#### Scenario: The inactive rule
- **WHEN** the frame is split with `C-x 2`
- **THEN** only the inactive mode line has a 1 px rule, inside its height along its top edge

### Requirement: The echo area is always shown
The frame SHALL end with one echo area line, as high as a mode line, showing the core's message or blank when there is none, so messages never resize the windows.

#### Scenario: A message appears
- **WHEN** `C-g` shows `Quit`
- **THEN** `Quit` is drawn in the echo area and the windows keep their sizes

### Requirement: The workspace modeline is drawn above every window
The frontend SHALL draw one workspace modeline row above the window tree, its background spanning the frame edge to edge while its text keeps the 12 px inset, in the editor's font at the workspace font size and that size's line height plus 4 px, rounded, showing the project name bold, ` › ` in the `separator` face and the workspace name, in the `workspace-bar` face. The windows SHALL be laid out below it, so it never covers buffer text, in windowed and fullscreen frames alike.

#### Scenario: One bar above the splits
- **WHEN** the frame shows two side-by-side windows in workspace `main` of project `home`
- **THEN** one row reading `home › main` is drawn above both windows, from the frame's left edge to its right, and the windows start below it

#### Scenario: Design size
- **WHEN** `workspace_font_size` is 13 and `line_height` is 1.4
- **THEN** the workspace modeline is 22 px high

## REMOVED Requirements

### Requirement: The frame keeps a gap at its edges
**Reason**: The approved design runs windows and bars edge to edge and insets their text instead.
**Migration**: See `Text is inset from the frame's edges`.

## ADDED Requirements

### Requirement: Text is inset from the frame's edges
Mode lines, the echo line, minibuffer rows, key hint rows and the debug stats SHALL draw their text 12 px from the frame's or window's left edge. Their backgrounds and windows SHALL reach the frame's edges, with no gap.

#### Scenario: Minibuffer at the corner
- **WHEN** `M-x` is typed
- **THEN** the prompt's first letter starts 12 px from the frame's left edge

#### Scenario: A window fills the frame
- **WHEN** one window is shown
- **THEN** it spans the frame's whole width and its mode line text starts 12 px from the left edge

### Requirement: Mode lines and the echo line share a height
Mode lines and the echo line SHALL be the line height plus 5 px, rounded, with their text centred in that height.

#### Scenario: Design size
- **WHEN** the font size is 15 and `line_height` is 1.4
- **THEN** mode lines and the echo line are 26 px high

### Requirement: A divider separates side-by-side windows
The 1 px gap between side-by-side windows SHALL be drawn in the `divider` face's background. Stacked windows SHALL have no divider, and the frame SHALL draw no other borders.

#### Scenario: Three windows
- **WHEN** a frame holds two windows side by side above a third
- **THEN** a 1 px `divider` stands between the two, down their full height with their mode lines, and none above the third

### Requirement: Ligatures are off
All text SHALL be shaped with contextual alternates off, with no setting, so `->` and `!=` draw as separate characters.

#### Scenario: A ligature font
- **WHEN** the font is Cascadia Code and a line holds `->`
- **THEN** `-` and `>` are drawn as two characters
