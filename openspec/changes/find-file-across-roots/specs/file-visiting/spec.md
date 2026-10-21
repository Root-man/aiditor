# Spec Delta

## ADDED Requirements

### Requirement: C-x C-f previews the selected file
`find-file` SHALL preview the selected candidate's file in the window it was opened from, as a previewing prompt does; a selected directory SHALL show that window as it was.

#### Scenario: Selecting a file
- **WHEN** `C-x C-f` lists a directory holding `b.txt` and `b.txt` is selected
- **THEN** the window shows `b.txt`'s text while the prompt is open

#### Scenario: Selecting a directory
- **WHEN** a subdirectory is selected
- **THEN** the window shows what it showed before the prompt
