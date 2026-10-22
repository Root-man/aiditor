# Spec Delta

## Purpose

Resolving a buffer whose unsaved edits conflict with a change to its file on disk: marking it, asking when it is focused or saved, and keeping mine, taking the disk's version or comparing the two.

## ADDED Requirements

### Requirement: A modified buffer whose file changes is conflicted
When a check finds a modified buffer's file present with a hash other than the buffer's disk hash, the buffer SHALL become conflicted with that disk version, keeping its text, modified flag and disk hash. The conflict SHALL end when a check finds the file back at the buffer's disk hash, when the file is found deleted, and when the buffer is reverted from or saved to the file. A save refused because the file changed on disk SHALL check the file, so a change the watcher missed makes the buffer conflicted.

#### Scenario: Another program writes a modified buffer's file
- **WHEN** `a.txt` is edited, another program rewrites it and the watcher reports it
- **THEN** the buffer keeps its text and stays modified, and its mode line shows `[conflict]`

#### Scenario: The file goes back
- **WHEN** a conflicted buffer's file is rewritten with the content the buffer last loaded, and reported
- **THEN** the buffer is no longer conflicted

#### Scenario: A change the watcher missed
- **WHEN** the file of a modified buffer changes on disk without a report and `C-x C-s` is pressed
- **THEN** the save is refused as a change on disk and the buffer becomes conflicted

### Requirement: Focusing or saving a conflicted buffer asks
When a conflicted buffer becomes the selected window's buffer, by any command or click, and no minibuffer is open, the editor SHALL ask `<path> changed on disk: ` with the choices `Compare`, `Take disk` and `Keep mine`, once per disk version. It SHALL NOT ask when the conflict arises while the buffer is already selected, until it is selected again. `C-x C-s` in a conflicted buffer SHALL ask the same question every time instead of saving. `M-x resolve-conflict` SHALL ask it for the current buffer. Quitting the question SHALL leave the buffer conflicted.

#### Scenario: Switching to a conflicted buffer
- **WHEN** a conflicted buffer is switched to with `C-x b`
- **THEN** the minibuffer asks `<path> changed on disk: ` with `Compare`, `Take disk` and `Keep mine`

#### Scenario: Asked once on focus
- **WHEN** the question asked on focus is quit with `C-g`, and the user switches away and back
- **THEN** no question is asked, and the mode line still shows `[conflict]`

#### Scenario: Conflicted while selected
- **WHEN** the selected buffer becomes conflicted
- **THEN** no question is asked until the buffer is saved or selected again

### Requirement: Keep mine overwrites the disk version on save
Answering `Keep mine` SHALL end the conflict with the buffer's text kept and the disk version taken as the save's base, so the next save overwrites it; answered from `C-x C-s`, the save SHALL go ahead.

#### Scenario: Keep mine from a save
- **WHEN** `C-x C-s` in a conflicted buffer is answered `Keep mine`
- **THEN** the file holds the buffer's text, the buffer is unmodified and not conflicted

#### Scenario: Keep mine on focus
- **WHEN** the question asked on focus is answered `Keep mine`, then `C-x C-s` is pressed
- **THEN** the save writes the buffer's text without asking

### Requirement: Take disk reverts, keeping the unsaved text in undo
Answering `Take disk` SHALL revert the buffer from its file as `revert-buffer` does, ending the conflict; one `C-/` SHALL bring the unsaved text back.

#### Scenario: Take disk
- **WHEN** a conflicted buffer's question is answered `Take disk`
- **THEN** the buffer holds the file's text, is unmodified and not conflicted, and `C-/` brings back the edited text

### Requirement: Compare shows the diff
Answering `Compare` SHALL show the diff of the buffer against its file as `diff-buffer-with-file` does, and leave the buffer conflicted.

#### Scenario: Compare
- **WHEN** a conflicted buffer's question is answered `Compare`
- **THEN** `*Diff*` shows the diff in another window, the buffer stays selected and conflicted
