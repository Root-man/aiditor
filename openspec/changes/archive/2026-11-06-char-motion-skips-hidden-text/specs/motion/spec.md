## MODIFIED Requirements

### Requirement: Char motion crosses lines and stops at the ends
`forward-char` and `backward-char` SHALL move by shown characters, a newline counting as one. Invisible text with an empty placeholder SHALL take no step, and invisible text with a placeholder, or a range with a display property, SHALL take one, so point never stops inside it. A motion that would pass an end of the buffer SHALL stop there and report `End of buffer` or `Beginning of buffer`.

#### Scenario: Over a markdown link
- **WHEN** point is 0 in `[ab](u) c` with `[` and `](u)` invisible with no placeholder, and `C-f` is pressed three times
- **THEN** point is at 2, then 3, then 8, before `c`

#### Scenario: Over a fold
- **WHEN** point is before a fold with placeholder `…` and `C-f` is pressed
- **THEN** point is right after the fold

#### Scenario: Across a line end
- **WHEN** point is at the end of the first line of `ab\ncd` and `C-f` is pressed
- **THEN** point is at the start of the second line

#### Scenario: Past the end
- **WHEN** point is 1 in `abc` and `C-u 5 C-f` is pressed
- **THEN** point is 3 and the message is `End of buffer`
