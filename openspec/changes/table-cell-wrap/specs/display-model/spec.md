# Spec Delta

## MODIFIED Requirements

### Requirement: Lines carry their decorations
A line decoration property SHALL apply to every display line its range touches. A decoration SHALL be a background color for the whole line, a rule beside it in a color, a 1 px border at the line's top or bottom in a color, a 1 px horizontal rule through the middle of the line's empty columns in a color, or a table line: the columns of its cell boundaries, whether it is its table's first line and whether it is its row's last.

#### Scenario: A code block background
- **WHEN** lines 1 and 2 have a background decoration
- **THEN** those display lines carry it and lines 0 and 3 do not

#### Scenario: Borders and a horizontal rule
- **WHEN** line 1 has a top border decoration and line 2 a horizontal rule decoration
- **THEN** display line 1 carries the border and display line 2 the rule, and no other line carries either

#### Scenario: A table row
- **WHEN** line 1 has a table decoration with boundaries 0, 3 and 7, marked first and last
- **THEN** display line 1 carries it with those boundaries, shifted left by the line's horizontal offset, still marked first and last
