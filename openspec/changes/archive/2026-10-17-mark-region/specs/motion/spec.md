# Spec Delta

## MODIFIED Requirements

### Requirement: Buffer ends
`beginning-of-buffer` and `end-of-buffer` SHALL move point to the start or end of the buffer. With a numeric prefix argument N (plain `C-u` counts as none) they SHALL instead move to N tenths of the buffer's length from the start or the end, then to the start of the next line. Unless the mark is active or the prefix is a plain `C-u`, they SHALL first push the mark at the old point without activating it and show `Mark set`.

#### Scenario: To the ends
- **WHEN** point is 2 in `ab\ncd` and `M->` then `M-<` is pressed
- **THEN** point is first 5, then 0, the mark is 5 and inactive, and the mark ring holds 2

#### Scenario: Tenths from the start
- **WHEN** the buffer is ten lines of `123456789\n`, 100 chars, and `C-u 5 M-<` is pressed
- **THEN** point is 60, the start of the line after the one holding char 50

#### Scenario: An active region keeps its mark
- **WHEN** `C-SPC` is pressed at 2 in `ab\ncd` and then `M->`
- **THEN** point is 5 and the mark is still 2 and active
