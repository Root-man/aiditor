# settings Specification

## Purpose

Loading the editor's TOML settings from bundled defaults and the user's config dir, validating them, reporting errors and reloading them at runtime.

## Requirements

### Requirement: User settings override bundled defaults
The editor SHALL ship bundled default settings and SHALL read the user's `settings.toml` from the platform config dir, with each key the user sets overriding the default and every other key keeping its default. A missing user file SHALL mean the defaults alone.

#### Scenario: Partial user file
- **WHEN** the user file sets only the font size
- **THEN** the font size is the user's and the font family and theme are the defaults

#### Scenario: No user file
- **WHEN** no user settings file exists
- **THEN** the bundled defaults apply and no error is reported

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

### Requirement: Settings errors stay visible until the next key
A settings error SHALL be shown on one line at the bottom of the frame and SHALL clear on the next key.

#### Scenario: Cleared by a key
- **WHEN** a settings error is shown and any key is pressed
- **THEN** the error line is gone

### Requirement: Watched file limit configuration
The bundled `max_watched_files` SHALL be 10000. Settings and project files SHALL accept a nonnegative integer, with a project's explicit value overriding settings and omission inheriting settings. Zero SHALL allow no root watches. Invalid types and negative values SHALL reject the configuration through its existing error flow. Existing project commands SHALL preserve the override and its comments.

#### Scenario: Project override
- **GIVEN** settings limit one and a project override two with two kept files and one directory
- **WHEN** the project opens
- **THEN** it is watched using limit two

#### Scenario: Settings reload
- **GIVEN** a project inherits the global limit and is refused
- **WHEN** settings reload raises the limit enough for its counts
- **THEN** its roots become watched

### Requirement: Runtime configuration reload reports completion
After an accepted runtime configuration reload has applied settings, keymaps and project definitions without diagnostics, the editor SHALL show `Configuration reloaded` in the echo area and log it to `*Messages*`. The message SHALL follow the normal echo-area lifetime. Completion SHALL refer to configuration application, without waiting for background indexing or watch setup. A reload with read, parse, validation or key-binding diagnostics SHALL retain its diagnostic messages and existing per-component fallback behavior instead of showing the success message. Superseded, duplicate or otherwise discarded results SHALL NOT announce completion. Startup loading SHALL NOT announce runtime reload completion.

#### Scenario: Reload succeeds
- **GIVEN** valid configuration files and an editor running with earlier settings
- **WHEN** `C-x C-r` completes its accepted reload
- **THEN** the new configuration is in effect and `Configuration reloaded` appears in the echo area and once in `*Messages*`

#### Scenario: Reload through M-x
- **GIVEN** valid configuration files
- **WHEN** `M-x reload-settings` completes
- **THEN** the same completion message appears

#### Scenario: Invalid configuration retains useful diagnostics
- **GIVEN** a reload with invalid settings, a valid keymap and a valid project definition
- **WHEN** the accepted bundle is applied
- **THEN** the previous settings remain, the other valid components apply, and the echo area shows the settings diagnostic rather than a success message

#### Scenario: Key-binding diagnostics also prevent success confirmation
- **GIVEN** a syntactically valid keymap containing an invalid binding that produces a diagnostic
- **WHEN** the accepted reload is applied
- **THEN** the keymap diagnostic remains visible and the log records that diagnostic rather than `Configuration reloaded`

#### Scenario: Only the latest accepted reload announces completion
- **GIVEN** one reload is pending and a second reload has been requested
- **WHEN** the old result arrives, followed by the successful latest result and a duplicate of that result
- **THEN** the latest configuration is in effect and the completion message is logged exactly once

#### Scenario: A prior unrelated message is not a reload failure
- **GIVEN** another command has reported an error while a reload was pending
- **WHEN** the accepted reload applies without diagnostics
- **THEN** `Configuration reloaded` is shown and the earlier error remains available in `*Messages*`

#### Scenario: Completion message clears normally
- **GIVEN** the echo area shows `Configuration reloaded`
- **WHEN** the user presses a key
- **THEN** the message clears before that action is handled

### Requirement: Grep worker configuration
The bundled `grep_workers` SHALL be 1. Settings SHALL accept a positive integer; zero, negative and non-integer values SHALL reject the settings file through the existing error flow, keeping the last valid settings, or the bundled defaults at startup. A project grep SHALL use the count in effect when it starts, so a successful reload applies to the next search and a running search keeps its count.

#### Scenario: Default
- **WHEN** `settings.toml` does not set `grep_workers`
- **THEN** project grep searches with one worker

#### Scenario: Invalid count
- **WHEN** `settings.toml` sets `grep_workers = 0`, `-2` or `"many"`
- **THEN** the settings error names `grep_workers` and the previous settings stay

#### Scenario: Reload applies to the next search
- **GIVEN** a search started with `grep_workers = 1`
- **WHEN** `grep_workers = 4` is reloaded and a new search starts
- **THEN** the new search carries 4 workers
