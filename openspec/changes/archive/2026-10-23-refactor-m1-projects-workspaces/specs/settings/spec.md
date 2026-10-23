# Spec Delta

## MODIFIED Requirements

### Requirement: C-x C-r reloads settings
`C-x C-r` SHALL read the user settings file again and apply it, and the frame SHALL redraw with the new values. Runtime reads of settings, keymaps and project definitions, including filesystem validation, SHALL run without blocking editor input. One reload's accepted results SHALL be applied in settings, keymap, then project order, preserving each component's existing validation and fallback behavior. When a newer reload is requested before an earlier one completes, the earlier results SHALL be discarded and the latest request SHALL be serviced. A reload snapshot taken before a newer successful project edit SHALL NOT replace that edit; the editor SHALL obtain a fresh project snapshot instead.

#### Scenario: Font size change takes effect
- **GIVEN** the font size in effect is 14
- **WHEN** the user file's font size is changed to 18 and `C-x C-r` is pressed
- **THEN** the font size in effect becomes 18

#### Scenario: Typing during reload
- **GIVEN** a runtime reload is waiting on project-directory reads
- **WHEN** the user types into the current buffer
- **THEN** the text appears before those reads finish

#### Scenario: A newer reload supersedes an older one
- **GIVEN** a reload is still reading and the configuration is edited again
- **WHEN** another reload is requested and both reads eventually finish
- **THEN** only the latest requested bundle is applied, in settings, keymap, then project order

#### Scenario: One invalid component
- **GIVEN** a reload finds invalid settings, a valid keymap and valid projects
- **WHEN** the accepted reload is applied
- **THEN** the previous settings remain, the settings error is reported, and the valid keymap and projects are applied

#### Scenario: A project edit finishes during reload
- **GIVEN** a reload has read an older project definition
- **WHEN** a project edit succeeds before that reload is applied
- **THEN** the newer edited definition remains usable while a fresh project snapshot is obtained
