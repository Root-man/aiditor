## ADDED Requirements

### Requirement: The diff is highlighted as the file is
The diff view SHALL highlight code as the file's own buffer would. Unchanged and added lines SHALL carry the faces the new side's buffer gives them, added lines with their syntax faces beside the added face's bar; the read-only buffer holding a `Staged` diff's index text SHALL take the file's grammar. Removed rows SHALL take their faces from a parse of the whole old text with the file's grammar, on top of the removed face, so a removed line inside a multi-line string or comment is coloured as one; old text over the size limit SHALL be parsed off the core thread, its rows drawn in the removed face alone until the parse arrives. Rows SHALL follow a theme change. A file in a language without a grammar SHALL show plain text in the diff faces.

#### Scenario: Old and new tokenise differently
- **GIVEN** a Rust file whose old text opens a block comment that the new text deleted
- **WHEN** its diff is shown
- **THEN** a removed line from inside the old comment has the comment face over the removed face, and the same text on the new side has code faces

#### Scenario: A staged diff
- **WHEN** a Rust file's `Staged` diff is shown
- **THEN** its index buffer's keywords have the keyword face

#### Scenario: A large old side
- **GIVEN** old text over the size limit
- **WHEN** its diff is shown
- **THEN** its removed rows are in the removed face alone until the parse arrives, then coloured

#### Scenario: No grammar
- **WHEN** the diff of a file with no known language is shown
- **THEN** removed rows are plain text in the removed face
