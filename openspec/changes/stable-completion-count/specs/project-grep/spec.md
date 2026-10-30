# Spec Delta

## ADDED Requirements

### Requirement: Streamed hits leave the list in place
A batch of hits added to the running search SHALL leave the selected hit and the shown rows as they were. Only the new search's first answer SHALL return the selection to the first hit.

#### Scenario: A batch arrives after the selection moved
- **WHEN** the running search has listed hits, `C-n` is typed and another batch arrives
- **THEN** the same hit stays selected and the same rows are shown
