## ADDED Requirements

### Requirement: Other commands leave the view first
While the diff view is open, any command or service answer that would show another buffer in a window or change the window tree SHALL quit the view first, as `q` does, and then run in the restored layout: switching to or visiting a buffer (`C-x b`, `C-x C-f`, `C-x p f`, a grep hit or a link followed, a jump back or forward to another buffer, a session, terminal, list or notification's buffer shown), showing a buffer in another window, and splitting or deleting windows (`C-x 2`, `C-x 3`, `C-x 0`, `C-x 1`). Editing and saving the shown file, `C-x o`, the minibuffer and its previews, the view's own commands and its showing of a diff's buffers, switching to the list itself, killing a buffer and showing another workspace SHALL leave the view open. A buffer that quitting the view kills SHALL not be shown.

#### Scenario: Visiting a file from either window
- **GIVEN** the view is open with the list or the diff window selected
- **WHEN** `C-x p f`, `C-x C-f` or `C-x b` picks another buffer
- **THEN** the saved layout is back, the list is gone and the buffer shows in the restored selected window

#### Scenario: Changing the window tree
- **GIVEN** the view is open
- **WHEN** `C-x 0`, `C-x 1`, `C-x 2` or `C-x 3` runs
- **THEN** the view is quit and the command acts on the restored layout's selected window

#### Scenario: Showing a buffer in another window
- **GIVEN** the view is open
- **WHEN** an answer shows a buffer in another window
- **THEN** the view is quit and the buffer shows beside the restored selected window

#### Scenario: Working in the view
- **GIVEN** the view is open with a file's diff shown
- **WHEN** the file is edited and saved, `C-x o` moves between the two windows, or the view's keys move between files
- **THEN** the view stays open
