# Spec Delta

## ADDED Requirements

### Requirement: Swiper result rows fit the available width
Each displayed Swiper result SHALL contain only the portion of its line-number prefix and source text that fits the completion area's available character columns, using the editor's existing column model. Clipping SHALL preserve valid Unicode text and aligned syntax styles. Filtering, selected-line identity, preview and acceptance SHALL continue to use the full source line regardless of clipping. Resizing the completion area or changing the font size SHALL update the displayed portion without changing the query or selected line. A row with no available columns SHALL have an empty presentation while retaining its selection and navigation behavior. Clipped results SHALL retain the source snapshot, complete syntax context, current theme and selection-background behavior defined by the other Swiper requirements.

#### Scenario: A long line with a match beyond the displayed text
- **GIVEN** a long source line whose only match is beyond the displayed columns
- **WHEN** Swiper filters for that match
- **THEN** the line is listed within the available width, and preview and acceptance navigate to the full line's actual match

#### Scenario: Resizing a result
- **GIVEN** a selected Swiper result is clipped at the completion area's edge
- **WHEN** the window is widened, narrowed or its font size changes
- **THEN** the displayed portion follows the new column budget and the same source line stays selected

#### Scenario: A narrow line-number prefix
- **GIVEN** the completion area is narrower than a result's padded line-number prefix, including zero available columns
- **WHEN** that result is selected and accepted
- **THEN** its presentation fits the available width and navigation still targets the correct source line

#### Scenario: Unicode at the clipping boundary
- **GIVEN** a source result contains multibyte text with syntax styles at the right edge
- **WHEN** the row is clipped
- **THEN** it contains valid text, and each displayed syntax span covers the corresponding displayed characters

#### Scenario: Snapshot context survives clipping
- **GIVEN** long result lines depend on multiline syntax, injected languages or local definitions outside the displayed rows
- **WHEN** filtering, scrolling, a source edit or a theme reload occurs while Swiper is open
- **THEN** the clipped text keeps the snapshot's correct syntax context, uses the current theme, and retains the selected row's background
