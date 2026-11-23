# Spec Delta

## ADDED Requirements

### Requirement: A root's branches and tags
Given roots, the file service SHALL answer, for each, nothing when it is in no repository, else its repository's top directory and the short names of its local branches and tags, or git's error.

#### Scenario: Branches and tags
- **GIVEN** a repository with branches `main` and `topic` and a tag `v1`, and a directory outside git
- **WHEN** both are asked for
- **THEN** the repository answers its top directory and `main`, `topic` and `v1`, and the other directory answers nothing
