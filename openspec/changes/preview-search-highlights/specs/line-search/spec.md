# Spec Delta

## ADDED Requirements

### Requirement: Swiper preview match faces
While Swiper previews a selected line, its window SHALL draw every visible nonempty positive match on a qualifying line with `match`, and the first match on the selected line with `match-current`. Matching SHALL retain Swiper's ordered words, invalid-regexp literal fallback, exclusions and smart case. Empty and exclusion-only queries SHALL use ordinary faces. Other windows SHALL keep ordinary faces. Accepting or aborting the prompt SHALL restore ordinary faces. Highlights SHALL be computed for display without text properties.

#### Scenario: Multiple matches and selection changes
- **GIVEN** visible lines `foo foo` and `foo bar`
- **WHEN** Swiper searches `foo` and selection moves to the second line
- **THEN** its first `foo` uses `match-current` and the first line's two matches use `match`

#### Scenario: Search semantics
- **GIVEN** lines contain ordered words, uppercase variants, a literal invalid regexp, and excluded words
- **WHEN** the corresponding Swiper queries preview a line
- **THEN** faces follow the same positive matching and exclusion rules as the listed lines

#### Scenario: Cleanup
- **GIVEN** a Swiper preview shows match faces
- **WHEN** the prompt is accepted or aborted
- **THEN** the buffer is drawn with its ordinary faces
