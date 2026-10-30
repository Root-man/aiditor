# Spec Delta

## MODIFIED Requirements

### Requirement: The gutter takes its width from the text
The gutter SHALL be 10px, then the digits of the buffer's line count in cells, then 12px wide, and text SHALL start after it. Lines SHALL wrap and scroll sideways in the columns left before the window's 12 px right inset, so the gutter never overlaps text. A window with no column left SHALL show no gutter.

#### Scenario: Growing with the line count
- **WHEN** a nine-line buffer in 8 px cells gets a tenth line
- **THEN** the gutter grows from 30 to 38 px

#### Scenario: Wrapping in what is left
- **WHEN** a nine-line file is shown in a window 122 px wide in 8 px cells
- **THEN** rows wrap in 10 columns less the marker's

#### Scenario: Drawn beside the text
- **WHEN** a file window is drawn
- **THEN** each number's right edge is 12 px before the text's left edge, and the cursor stays over its char

#### Scenario: Too narrow
- **WHEN** a window is no wider than the gutter, one cell and the right inset
- **THEN** it shows no gutter
