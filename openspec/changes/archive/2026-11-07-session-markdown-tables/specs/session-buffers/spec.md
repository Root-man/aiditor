# Spec Delta

## ADDED Requirements

### Requirement: Tables render as drawn text
A GFM pipe table at the top level of an agent message block SHALL be replaced in the buffer by drawn text: one line for the header and one for each body row, without the delimiter row. Each cell SHALL be a space, its text padded to its column's width and a space. A column's width SHALL be its widest cell's shown text, measured in chars. Cells SHALL align left, centre or right as the delimiter row says. Each line SHALL carry the table line decoration.

#### Scenario: Columns and alignment
- **WHEN** the agent streams `| a | b |\n|---|--:|\n| x | 10 |\n` and a frame is built
- **THEN** the block's lines read ` a   b ` and ` x  10 `, and both carry the table decoration

#### Scenario: A ragged table
- **WHEN** a table's header has three cells, one body row two and another four
- **THEN** each drawn line has three cells: the short row ends in an empty cell and the long row's fourth cell is dropped

#### Scenario: A nested table stays text
- **WHEN** the agent streams a table inside a block quote or a list item
- **THEN** its lines stay as written, pipes and delimiter row included

#### Scenario: A table still streaming
- **WHEN** a table's header and delimiter row have arrived, a frame is built, then a row arrives and another frame is built
- **THEN** the first frame shows the header line alone, and the second also shows the row, with columns widened to fit it

### Requirement: Table cells render their markup
Inline markup in a table cell SHALL be rendered with the faces messages use and left out of the drawn text and of the column's width; block markup in a cell SHALL stay as written. The header line SHALL be bold on the `session-block` background across the table's width. Links in cells SHALL be drawn and open with `RET` as in other message text; a markdown link SHALL show only its label, and `RET` on the label SHALL act on its destination.

#### Scenario: Markup in a cell
- **WHEN** a table's cell holds `**hot** path` and the column's other cells are shorter
- **THEN** the cell's drawn text is `hot path` with `hot` bold, and the column is 8 chars wide

#### Scenario: Block markers in a cell
- **WHEN** a table's cells hold `#`, `# *x*` and a code fence
- **THEN** they draw as `#`, `# x` with `x` italic, and the fence as written

#### Scenario: The header
- **WHEN** a table is drawn
- **THEN** its header line's text is bold and has the `session-block` background from the table's first column to its last

#### Scenario: A link in a cell
- **WHEN** a table's cell holds `[docs](https://x.dev)` and `RET` is typed on `docs`
- **THEN** the cell's drawn text is `docs` in `link`, and `https://x.dev` opens in the system's browser

#### Scenario: A link in a cell to another scheme
- **WHEN** a table's cell holds `[sh](file:///bin/sh)` and `RET` is typed on `sh`
- **THEN** the echo area says `Not a web link: file:///bin/sh` and nothing opens

### Requirement: A message's time avoids its table rows
A live agent message's clock time SHALL go at the end of its first line that is neither blank nor a table row. A message whose lines are all table rows or blank SHALL get no time.

#### Scenario: A reply starting with a table
- **WHEN** a live reply `| a |\n|---|\n| b |\n\nAfter.` ends at 14:01:00
- **THEN** its lines read ` a `, ` b `, an empty line and `After. 14:01:00`

#### Scenario: A reply of a table alone
- **WHEN** a live reply `| c |\n|---|\n| d |` ends
- **THEN** its lines read ` c ` and ` d ` with no time

## MODIFIED Requirements

### Requirement: Markup stays in the text
Rendering SHALL NOT change the buffer's text outside tables: search SHALL match hidden markup and copying or killing a rendered message SHALL yield its markdown. A table SHALL be the exception: search, copy and kill SHALL see its drawn text, and its markdown SHALL NOT be in the buffer.

#### Scenario: Swiper finds hidden markup
- **WHEN** a reply `**bold**` is rendered and swiper searches for `**b`
- **THEN** it matches the reply's line

#### Scenario: Copy yields markdown
- **WHEN** a rendered reply `**bold**` is marked whole and `M-w` is typed
- **THEN** the kill ring's top is `**bold**`

#### Scenario: Copy yields a drawn table
- **WHEN** the table `| a | b |\n|---|--:|\n| x | 10 |` is rendered, its two lines are marked whole and `M-w` is typed
- **THEN** the kill ring's top is ` a   b \n x  10 \n`

#### Scenario: Search sees a drawn table
- **WHEN** that table is rendered and swiper searches for `| x`, then for `x  10`
- **THEN** the first search matches nothing and the second matches the table's second line
