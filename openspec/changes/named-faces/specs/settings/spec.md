## MODIFIED Requirements

### Requirement: M0 settings cover font and theme
The settings SHALL hold a font family, a font size, `option_as_meta`, a `theme` name and a `themes` table of named themes, and the frame SHALL draw with them. An unset font family SHALL mean the platform's monospace font. `option_as_meta` SHALL default to true and SHALL only matter on macOS.

`theme` SHALL pick a `[themes.<name>]` table, merged key by key over the bundled `themes.default`, so a theme lists only what it changes. A theme SHALL hold a 16-colour `ansi` palette, `[themes.<name>.faces]` and `[themes.<name>.syntax]`. A face SHALL be a table of `fg`, `bg`, `bold`, `italic` and `underline`, an unset attribute taking what is underneath. `themes.default` SHALL define `default`, `cursor`, `echo`, `separator`, `mode-line`, `mode-line-inactive`, `mode-line-dim`, `workspace-bar`, `workspace-current`, `divider`, `line-number`, `line-number-current`, `hl-line`, `region`, `match`, `match-current`, `completion-match`, `completion-selected`, `annotation`, `prompt`, `directory`, `grep-file`, `grep-line-number`, `vcs-branch`, `status-modified`, `status-waiting`, `status-running`, `status-failed`, `status-info`, `session-user`, `session-thought`, `session-block`, `session-code`, `diff-added`, `diff-removed`, `diff-hunk`, `key-hint`, `permission`, `link`, `attachment` and `message-at-point`.

#### Scenario: Font size drawn
- **WHEN** the font size setting is 18
- **THEN** buffer text is drawn at 18 px

#### Scenario: Region color
- **WHEN** the user file sets `bg` of the current theme's `region` face
- **THEN** the display model highlights the region in that color

#### Scenario: Mode line colors
- **WHEN** the user file sets `[themes.default.faces] mode-line = { bg = 0x111111 }` and `C-x C-r` reloads it
- **THEN** the selected window's mode line is drawn on it, keeping the bundled `fg`, and the other mode lines keep the default inactive face

#### Scenario: Option as Meta by default
- **WHEN** no user file sets `option_as_meta`
- **THEN** `option_as_meta` is true

#### Scenario: ANSI palette
- **WHEN** the current theme sets `ansi` to 16 colours
- **THEN** terminal cells in ANSI colour 1 use its second colour

#### Scenario: Code block colour
- **WHEN** the current theme sets `bg` of `session-block`
- **THEN** code blocks in session buffers are drawn on that background

#### Scenario: Switching themes
- **WHEN** the user file defines `[themes.mine.faces] default = { bg = 0x000001 }`, sets `theme = "mine"` and `C-x C-r` reloads it
- **THEN** the frame draws on that background and every face `mine` doesn't list keeps its `themes.default` value

### Requirement: Invalid settings are rejected whole
A settings file that is not valid TOML, has a value of the wrong type, a font size outside 6 to 72, or a key the editor does not know SHALL be rejected. A `theme` that names no `[themes.<name>]` table, a face name or face attribute the editor does not know in any theme, selected or not, and the old `[theme]` table SHALL be rejected the same way. The editor SHALL show an error naming the problem and SHALL keep the settings it had before.

#### Scenario: Wrong type
- **WHEN** the user file sets the font size to `"big"` and the settings are reloaded
- **THEN** an error is shown and the previous font size stays in effect

#### Scenario: Unknown key
- **WHEN** the user file sets `font_sise = 16`
- **THEN** an error naming the unknown key is shown and the previous settings stay in effect

#### Scenario: Out of range
- **WHEN** the user file sets the font size to 0
- **THEN** an error is shown and the previous font size stays in effect

#### Scenario: Unreadable file
- **WHEN** the user file exists but cannot be read
- **THEN** an error is shown and the previous settings stay in effect

#### Scenario: A theme with no table
- **WHEN** the user file sets `theme = "nope"`
- **THEN** an error naming `nope` is shown and the previous settings stay in effect

#### Scenario: An unknown face
- **WHEN** a theme that isn't selected sets a face `mode-lin`
- **THEN** an error naming `mode-lin` is shown and the previous settings stay in effect

#### Scenario: An unknown attribute
- **WHEN** a theme sets `region = { background = 1 }`
- **THEN** an error naming `background` is shown and the previous settings stay in effect

#### Scenario: The old table
- **WHEN** the user file has a `[theme]` table
- **THEN** an error naming `theme` is shown and the previous settings stay in effect
