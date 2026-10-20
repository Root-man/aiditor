# Spec Delta

## ADDED Requirements

### Requirement: The completion list only grows while the minibuffer is open
While one minibuffer is open, the frame display SHALL carry as many completion rows as the most it has shown since it opened, and the frontend SHALL draw the rows past the shown matches empty, so the windows above keep their height. A newly opened minibuffer SHALL start at the rows its own matches need.

#### Scenario: Fewer matches keep the height
- **WHEN** `find-file` lists a directory with 10 entries and the text then moves to a directory with 5
- **THEN** the frame display's minibuffer lists 5 candidates in 10 rows, and the frontend's window area keeps its height

#### Scenario: The next minibuffer starts afresh
- **WHEN** a minibuffer that showed 10 rows closes and a prompt with 3 candidates opens
- **THEN** the frame display's minibuffer carries 3 rows
