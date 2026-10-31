# Spec Delta

## MODIFIED Requirements

### Requirement: The mode line marks the selected window
Each window's mode line SHALL be drawn on the theme's `mode-line` face's background for the selected window and on its `mode-line-inactive` face's for the others, with the left group's text at the left inset and the right group's ending at the right inset, each span in its own face. An inactive mode line SHALL have a 1 px rule along its top in the `mode-line` face's background.

#### Scenario: Two windows
- **WHEN** the frame is split with `C-x 2`
- **THEN** the upper mode line uses the `mode-line` face and the lower one `mode-line-inactive`

#### Scenario: The inactive rule
- **WHEN** the frame is split with `C-x 2`
- **THEN** only the inactive mode line has a 1 px rule, inside its height along its top edge

#### Scenario: The right group
- **WHEN** a window's mode line is drawn
- **THEN** its right group ends 12 px from the window's right edge
