# Spec Delta

## ADDED Requirements

### Requirement: One git update at a time
After `diff-stage`, `diff-unstage` or `diff-discard` sends git a step, from the list or from the diff window, the view SHALL be updating until git fails or the refreshed list arrives. While it is updating, these three commands SHALL do nothing but echo `Git is still updating`, in either window and for files or changes alike. A discard of a change, which edits the buffer and runs no git, SHALL be refused the same way. A failed update SHALL end the wait with git's message echoed, so the next command runs. A refresh with `g` SHALL NOT start the wait, and a refresh that brings the list in SHALL end it.

#### Scenario: A second stage while git runs
- **GIVEN** `s` was pressed on a file and git hasn't answered
- **WHEN** `s` is pressed on another file
- **THEN** `Git is still updating` is echoed and no second git step is sent

#### Scenario: Waiting for the refreshed list
- **GIVEN** git answered a stage with success and the refreshed status hasn't arrived
- **WHEN** `C-c s` is pressed on a change in the diff window
- **THEN** `Git is still updating` is echoed and no patch is built

#### Scenario: The refreshed list ends the wait
- **GIVEN** a stage succeeded
- **WHEN** the refreshed list arrives and `s` is pressed on another file
- **THEN** git is sent that file's stage

#### Scenario: A failure ends the wait
- **WHEN** git fails a stage with a message
- **THEN** the message is echoed and the next `s` sends git a step

#### Scenario: A refresh alone doesn't wait
- **GIVEN** `g` was pressed and its status hasn't arrived
- **WHEN** `s` is pressed on a file
- **THEN** git is sent that file's stage
