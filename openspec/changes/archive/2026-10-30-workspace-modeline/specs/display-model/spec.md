## ADDED Requirements

### Requirement: The frame display names the current project and workspace
The frame display SHALL carry the name of the current workspace's project and the current workspace's name for the workspace modeline, whatever buffer, window or minibuffer is current.

#### Scenario: Names follow the current workspace
- **WHEN** the current workspace is `main`, bound to project `home`
- **THEN** the frame display names project `home` and workspace `main`
- **WHEN** a workspace `w2` for project `other` becomes current and is renamed `w3`
- **THEN** it names project `other` and workspace `w3`
- **WHEN** `main` is made current again
- **THEN** it names `home` and `main` again

#### Scenario: Names stay while editing and prompting
- **WHEN** the window is split, another window selected, another buffer shown or the minibuffer opened
- **THEN** the frame display still names the current project and workspace

### Requirement: The workspace modeline names are truncated to the frame width
When `<project> › <workspace>` is wider than the frame's columns, the names SHALL be cut so the line fits: a name that fits in half the room left after ` › ` stays whole and the other takes the rest, otherwise each takes half, the workspace getting the odd column. A cut name ends in `…` and keeps at least that character when it has any room.

#### Scenario: Names that fit stay whole
- **WHEN** project `aiditor` and workspace `watcher-fix` are shown 40 columns wide
- **THEN** the names are `aiditor` and `watcher-fix`

#### Scenario: A long workspace name is cut
- **WHEN** project `Legacy crap` and a 60-character workspace name are shown 40 columns wide
- **THEN** the project stays `Legacy crap` and the workspace is cut to 26 characters ending in `…`

#### Scenario: Two long names are both cut
- **WHEN** two 30-character names are shown 23 columns wide
- **THEN** each is cut to 10 characters ending in `…`
