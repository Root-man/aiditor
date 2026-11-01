# Spec Delta

## ADDED Requirements

### Requirement: Swiper results retain source syntax faces
Swiper SHALL draw the source text in each visible result with the source snapshot's syntax faces, including colours, bold, italic and underline attributes. Line-number prefixes and text without syntax highlighting SHALL retain their ordinary face. Faces SHALL retain complete source context, including multiline constructs, injected languages and local references defined outside the visible results. Filtering and scrolling SHALL preserve correct faces on each displayed row. A selected row SHALL retain its selection background over any syntax background. Successful theme reloads SHALL update visible result faces. If the origin buffer changes during a prompt, syntax faces SHALL remain aligned with the snapshot text actually listed. Source changes SHALL retain the existing live preview and acceptance behaviour.

#### Scenario: Source colours and styles
- **GIVEN** a source buffer with styled keywords, strings and comments
- **WHEN** C-s lists its lines
- **THEN** the source text retains those faces and the line-number prefixes use their ordinary face

#### Scenario: Source context outside a result
- **GIVEN** matching lines inside multiline strings or comments, Markdown language fences or HEEx templates, or using locals defined outside those lines
- **WHEN** Swiper displays those result rows
- **THEN** their syntax faces retain the complete source context

#### Scenario: Filtering and scrolling
- **GIVEN** a source buffer with more result rows than the completion list displays
- **WHEN** the query changes or selection scrolls the list
- **THEN** each visible result retains its source syntax faces and the selected row keeps its selection background

#### Scenario: Theme reload
- **GIVEN** Swiper is showing highlighted results
- **WHEN** settings successfully reload a changed syntax palette or font style
- **THEN** the visible results use the new faces

#### Scenario: Unicode and source changes
- **GIVEN** Swiper lists a source snapshot containing multibyte characters
- **WHEN** the source buffer is edited or reverted while the prompt remains open
- **THEN** colours remain aligned with the displayed snapshot text, and preview and acceptance continue to follow the live buffer

#### Scenario: Plain buffer
- **GIVEN** a buffer without syntax highlighting
- **WHEN** C-s lists its lines
- **THEN** the result text uses its ordinary face
