# Spec Delta

## MODIFIED Requirements

### Requirement: The region is highlighted
When the mark is set and active, the text between point and mark SHALL have the theme's region color as its background, over any face's background.

#### Scenario: Region across a face
- **WHEN** the mark is active at 0 and point at 3 in `hello`
- **THEN** `hel` has the region background and `lo` the default face

#### Scenario: No mark
- **WHEN** the mark is unset
- **THEN** no span has the region background

#### Scenario: Inactive mark
- **WHEN** the mark is at 0 and inactive and point at 3 in `hello`
- **THEN** no span has the region background
