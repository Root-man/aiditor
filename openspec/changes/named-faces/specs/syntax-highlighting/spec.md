## MODIFIED Requirements

### Requirement: Theme syntax palette
The theme SHALL have a `syntax` table mapping capture names to faces, so a capture can be italic. A capture with no entry SHALL use the entry of its longest dotted prefix that has one (`function.method` falls back to `function`), and none means the default face. A user's `[themes.<name>.syntax]` entry SHALL override that entry only.

#### Scenario: User overrides one colour
- **WHEN** the user's settings set `[themes.default.syntax] keyword = { fg = 0xff0000 }`
- **THEN** keywords are drawn in 0xff0000 and strings keep the bundled colour

#### Scenario: Prefix fallback
- **WHEN** a capture is named `function.method` and the palette has only `function`
- **THEN** it is drawn in the `function` face

#### Scenario: Italic comments
- **WHEN** the theme sets `comment = { fg = 0x928374, italic = true }`
- **THEN** comments are drawn italic in that colour
