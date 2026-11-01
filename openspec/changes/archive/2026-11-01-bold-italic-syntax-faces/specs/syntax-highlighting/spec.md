## MODIFIED Requirements

### Requirement: Theme syntax palette
The theme SHALL have a `syntax` table mapping capture names to face tables with optional `fg`, `bg`, `bold`, `italic` and `underline` attributes; bare colours SHALL be rejected. A capture with no entry SHALL use the entry of its longest dotted prefix that has one (`function.method` falls back to `function`), and none means the default face. A user's `[themes.<name>.syntax]` overrides SHALL merge per attribute with that theme's defaults, preserving unspecified attributes and allowing `false` to disable inherited bold or italic. Settings reloads SHALL update syntax faces in already-open buffers.

#### Scenario: User overrides one colour
- **GIVEN** the bundled syntax palette
- **WHEN** the user's settings set `[themes.default.syntax] keyword = { fg = 0xff0000 }`
- **THEN** keywords are drawn in 0xff0000 and strings keep the bundled colour

#### Scenario: Prefix fallback
- **GIVEN** the palette has only `function`
- **WHEN** a capture is named `function.method`
- **THEN** it is drawn in the `function` face

#### Scenario: Italic comments
- **GIVEN** the theme sets `comment = { fg = 0x928374, italic = true }`
- **WHEN** a source buffer is drawn
- **THEN** comments are drawn italic in that colour

#### Scenario: Italic comments and bold keywords
- **GIVEN** the selected theme sets `comment = { fg = 0x928374, italic = true }` and `keyword = { bold = true }`
- **WHEN** a source buffer is drawn
- **THEN** comments are italic in that colour and keywords are bold

#### Scenario: Attribute overrides preserve other attributes
- **GIVEN** the default theme has italic comments and bold keywords
- **WHEN** the selected theme changes only the comment foreground and keyword foreground
- **THEN** the new colours are used while comments remain italic and keywords remain bold

#### Scenario: Disable styles on reload
- **GIVEN** an open source buffer using italic comments and bold keywords
- **WHEN** settings are reloaded with `comment = { italic = false }` and `keyword = { bold = false }` in the selected theme
- **THEN** comments and keywords use normal font styles while retaining their inherited colours

#### Scenario: Bare colour rejected
- **GIVEN** the currently accepted settings
- **WHEN** settings are reloaded with `comment = 0x928374` in any theme's syntax table
- **THEN** an error is reported and the previous settings remain active
