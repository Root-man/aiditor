# Spec Delta

## ADDED Requirements

### Requirement: Grep preview match faces
While a selected hit is actually previewed, its window SHALL draw every visible nonempty match with `match`, and the first match on the selected hit's line with `match-current`. Matching SHALL use the displayed results' regexp and ripgrep smart case, including while replacement results are pending. Other windows showing the same buffer SHALL keep their ordinary faces. Closing or aborting the prompt SHALL restore ordinary faces. Highlights SHALL be display overlays, not text properties.

#### Scenario: All visible matches and the selected match
- **GIVEN** a previewed file has multiple matches on the selected line and on another visible line
- **WHEN** a grep hit is selected
- **THEN** the selected line's first match uses `match-current` and every other visible match uses `match`

#### Scenario: Smart case and Unicode
- **GIVEN** a file holds `été Foo foo` and a grep for `foo` or `Foo`
- **WHEN** a hit is previewed
- **THEN** the lowercase query highlights both spellings, the uppercase query only `Foo`, and accented text keeps the match boundaries correct

#### Scenario: Replacement and delayed preview
- **GIVEN** old hits remain listed while another query searches or a selected file is still loading
- **WHEN** the frame is built
- **THEN** a shown old hit retains its displayed query's faces, and a file awaiting preview receives no premature overlay

#### Scenario: Split windows and prompt cleanup
- **GIVEN** two windows show the same file and grep previews in one
- **WHEN** the prompt is accepted or aborted
- **THEN** both windows use their ordinary faces, and during preview only the originating window had match faces

#### Scenario: Scrolled and wrapped text
- **GIVEN** a preview has scrolled vertically or horizontally, or wraps a matching line
- **WHEN** its visible text is built
- **THEN** visible portions of matches retain their corresponding match faces and regexp anchors refer to buffer lines
