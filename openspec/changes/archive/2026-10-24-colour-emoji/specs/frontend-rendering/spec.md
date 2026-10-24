## ADDED Requirements

### Requirement: Emoji are drawn
Emoji SHALL be drawn wherever the frontend draws text. When the colour emoji font can't be drawn, having neither colour bitmaps nor COLR v0 layers, emoji SHALL be drawn in the text's colour from another installed font that has them, such as Noto Emoji or Symbola. A colour emoji font that can be drawn SHALL be used as is.

#### Scenario: Crab on Fedora
- **WHEN** a file containing `🦀` is shown, the only colour emoji font is COLRv1 and Noto Emoji is installed
- **THEN** the crab is drawn, monochrome, in the foreground colour

#### Scenario: Crab with a drawable colour font
- **WHEN** a file containing `🦀` is shown on macOS
- **THEN** the crab is drawn in colour from Apple Color Emoji
