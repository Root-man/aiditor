# Spec Delta

## ADDED Requirements

### Requirement: A completing prompt can preview its selection
When a command declares a preview for its prompts, the window the minibuffer was opened from SHALL show the buffer the preview gives for the selected candidate, following every change of the selection, from the moment the list opens. A candidate the preview gives no buffer for, or no selection, SHALL show that window as it was. Previewing SHALL NOT make a buffer the most recent one. A question a command asks with its own choices SHALL NOT preview.

#### Scenario: Moving the selection previews
- **WHEN** a previewing prompt is opened from a window showing `a` and `C-n` selects a candidate whose buffer is `b`
- **THEN** that window shows `b`, and the minibuffer stays selected

#### Scenario: No buffer shows the window as it was
- **WHEN** the text matches no candidate
- **THEN** the window shows `a` again, with its old point

### Requirement: Closing the minibuffer restores the previewed window
Closing a previewing minibuffer in any way (`C-g`, `RET`, a click, switching workspace) SHALL first put the window it was opened from back exactly as it was: its buffer, point, mark, whether the mark is active, mark ring and scroll. `RET` SHALL then run the command as it would without a preview.

#### Scenario: Preview then C-g
- **WHEN** a window has point, an active mark and a mark ring, a previewing prompt previews other buffers, and `C-g` is typed
- **THEN** the frame's windows are exactly as before the prompt opened

#### Scenario: RET runs the command after restoring
- **WHEN** a previewing prompt previews a candidate and `RET` is typed
- **THEN** the command runs with the candidate, from the window as it was before the prompt
