# Spec Delta

## ADDED Requirements

### Requirement: Candidates can carry annotations
A command that completes over candidates SHALL be able to annotate each one with text worked out when the minibuffer opens. The frame display SHALL carry each shown candidate's annotation, and the frontend SHALL draw it after the candidate. Annotations SHALL NOT be matched against the text.

#### Scenario: Annotations are not matched
- **WHEN** a command completes over `apple` annotated `zz` and `banana` annotated `none`, and `zz` is typed
- **THEN** the list is empty

#### Scenario: Annotations in the frame
- **WHEN** that command is run
- **THEN** the frame display's minibuffer lists `apple` with the annotation `zz` and `banana` with `none`
