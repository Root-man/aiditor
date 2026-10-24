## ADDED Requirements

### Requirement: Clipboard keys follow the platform
The bundled keymaps SHALL bind copy, cut and paste to `s-c`, `s-x` and `s-v` on macOS, and to `C-S-c`, `C-S-x` and `C-S-v` on Linux, where the desktop takes Super keys; each platform SHALL leave the other's keys unbound. They SHALL live in a platform keymap file bound after the shared one, so the user keymap still overrides them.

#### Scenario: Paste on Linux
- **WHEN** on Linux text was copied and `C-S-v` is pressed in a text buffer
- **THEN** the text is yanked, and `s-v` is undefined
