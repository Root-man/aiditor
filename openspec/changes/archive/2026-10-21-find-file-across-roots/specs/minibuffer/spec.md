# Spec Delta

## MODIFIED Requirements

### Requirement: A completing prompt can preview its selection
When a command declares a preview for its prompts, the window the minibuffer was opened from SHALL show the buffer the preview gives for the selected candidate, following every change of the selection, from the moment the list opens. A candidate the preview gives no buffer for, or no selection, SHALL show that window as it was. Previewing SHALL NOT make a buffer the most recent one. A question a command asks with its own choices SHALL NOT preview, unless the command declares no prompts, so that question is its only one.

#### Scenario: Moving the selection previews
- **WHEN** a previewing prompt is opened from a window showing `a` and `C-n` selects a candidate whose buffer is `b`
- **THEN** that window shows `b`, and the minibuffer stays selected

#### Scenario: No buffer shows the window as it was
- **WHEN** the text matches no candidate
- **THEN** the window shows `a` again, with its old point

## ADDED Requirements

### Requirement: A preview can show a file not yet visited
A preview SHALL be able to name a file. A file visited in the current workspace SHALL show its buffer. Any other file SHALL be read off the core thread into a hidden, read-only preview buffer, the window keeping what it showed until the read answers; an answer for a file no longer selected SHALL be dropped. A file that is missing, binary or unreadable SHALL show the window as it was.

#### Scenario: Previewing an unvisited file
- **WHEN** a previewing prompt selects a file no buffer visits
- **THEN** once it is read, the window shows its text, and no buffer list offers the preview buffer

#### Scenario: A late read
- **WHEN** the selection moves on before a file's read answers
- **THEN** that answer is dropped and the window never shows that file

### Requirement: One preview buffer at a time
There SHALL be at most one preview buffer. It SHALL be killed when another file is read for preview and when the minibuffer closes, so `RET` visits the file as it would without a preview.

#### Scenario: Closing kills the preview buffer
- **WHEN** a file is previewed and `C-g` is typed
- **THEN** the window is as it was and no buffer is left for the file
