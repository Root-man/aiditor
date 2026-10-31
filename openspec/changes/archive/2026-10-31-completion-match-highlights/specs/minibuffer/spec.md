## ADDED Requirements

### Requirement: Fuzzy completion lists highlight matched characters
Every fuzzy completing prompt SHALL draw the matched characters of each visible candidate in the theme's `completion-match` face. Highlights SHALL reflect the pattern that produced the displayed matches, including smart case, normalization and multi-atom patterns, and follow query changes and scrolling. Unicode matches SHALL highlight complete grapheme clusters. An empty pattern SHALL show candidates in their normal face. Selected rows SHALL retain their selection background, and annotations SHALL retain their dimmed appearance separately from candidate highlights. Swiper and project grep retain their existing filtering and rendering.

#### Scenario: Command initials
- **GIVEN** the `M-x` prompt is open
- **WHEN** `pff` is typed
- **THEN** the `p`, first `f` and second `f` in `project-find-file` use `completion-match`

#### Scenario: Other fuzzy prompts
- **GIVEN** `project-find-file`, `find-file` or `C-x b` is completing
- **WHEN** a query matches a candidate
- **THEN** its matched characters use `completion-match`, including on the selected row, and annotations stay dimmed

#### Scenario: Unicode and multiple atoms
- **GIVEN** candidates contain accented text, decomposed accents or emoji
- **WHEN** a multi-atom fuzzy query matches them
- **THEN** the corresponding complete grapheme clusters are highlighted and the candidate text stays intact

#### Scenario: Updating the visible matches
- **GIVEN** a fuzzy completion list has more than ten matching candidates
- **WHEN** the query changes, the selection scrolls the list or the query is cleared
- **THEN** each visible candidate's highlights reflect its displayed match pattern, with normal faces for the cleared query
