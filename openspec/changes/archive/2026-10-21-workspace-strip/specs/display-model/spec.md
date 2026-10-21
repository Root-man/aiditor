# Spec Delta

## ADDED Requirements

### Requirement: The frame title names the buffer, workspace and project
The frame display's title SHALL be the current buffer's name, followed by ` *` when that buffer is modified, then ` | `, the current workspace's name, ` | ` and the name of the project that workspace is bound to. The frontend SHALL show it as the window title.

#### Scenario: Title of an unmodified buffer
- **WHEN** the current workspace is `main`, bound to project `aiditor`, and shows unmodified `t.txt`
- **THEN** the title is `t.txt | main | aiditor`

#### Scenario: Title of a modified buffer
- **WHEN** `t.txt` is modified in that workspace
- **THEN** the title is `t.txt * | main | aiditor`

#### Scenario: Title follows the current workspace
- **WHEN** a second workspace `w2` is made for project `other` and becomes current
- **THEN** the title is `*scratch*<w2> | w2 | other`
- **WHEN** it is renamed to `w3`
- **THEN** the title is `*scratch*<w3> | w3 | other`
- **WHEN** the first workspace is made current again
- **THEN** the title names `main` and `aiditor` again
