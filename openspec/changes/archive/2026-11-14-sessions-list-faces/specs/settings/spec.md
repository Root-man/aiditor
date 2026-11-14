## MODIFIED Requirements

### Requirement: Quality and prompt-band faces
`themes.default` SHALL also define `quality-poor`, `quality-common`, `quality-uncommon`, `quality-rare`, `quality-epic`, `quality-legendary`, `quality-artifact`, `session-user-rule`, and the sessions list's `session-list-group`, `session-list-pinned`, `session-list-count`, `session-list-today`, `agent-claude` and `agent-codex`, and `session-user` SHALL set a foreground and bold besides its background. Each SHALL be overridable in a theme as every face is.

#### Scenario: Overriding a quality colour
- **WHEN** the user file sets `[themes.default.faces] quality-legendary = { fg = 0x123456 }` and `C-x C-r` reloads it
- **THEN** a session using a legendary model draws its model name in that colour in the selected window

#### Scenario: Bundled faces
- **WHEN** no user file is loaded
- **THEN** all fourteen faces have a foreground, and `session-user` has a background, a foreground and bold
