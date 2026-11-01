# line-search Specification

## Purpose
Swiper: searching the current buffer by listing its matching lines in the minibuffer, previewing the selected one and jumping to it, as ivy's swiper does.

## Requirements

### Requirement: C-s lists the buffer's lines
`C-s` (`swiper`) SHALL open the minibuffer at `Swiper: `, completing over every line of the current buffer in buffer order, never sorted. A final newline SHALL NOT add an empty last line. Each line SHALL be shown after its line number, left-aligned and padded to the width of the largest one, then a space; the number SHALL NOT be matched. The line at point SHALL be selected when the list opens.

#### Scenario: Opening lists every line
- **WHEN** in a buffer `apple\nbanana\ncherry\n` with point on `banana`, `C-s` is typed
- **THEN** the list shows `1 apple`, `2 banana` and `3 cherry` in that order with `2 banana` selected, and the count reads `2/3`

#### Scenario: Line numbers are not matched
- **WHEN** in a buffer of 12 lines where only line 12 holds `1`, `1` is typed at the prompt
- **THEN** only line 12 is listed

### Requirement: Swiper matches words in order
The input SHALL be split on spaces, a backslash-escaped space being a literal space, and a line SHALL match when the words appear on it in that order. Each word SHALL be a regular expression, or a literal string when it is not a valid one. Words after the first unescaped `!` SHALL exclude the lines any of them match. Matching SHALL ignore case while the input has no upper-case letter.

#### Scenario: Words in order
- **WHEN** the lines are `foo bar`, `bar foo` and `foobar`, and `foo bar` is typed
- **THEN** only `foo bar` and `foobar` are listed

#### Scenario: Exclusion
- **WHEN** the lines are `foo one`, `foo two`, and `foo !two` is typed
- **THEN** only `foo one` is listed

#### Scenario: Smart case
- **WHEN** the lines are `Foo` and `foo`, and `foo`, then `Foo`, is typed
- **THEN** both lines are listed for `foo`, and only `Foo` for `Foo`

#### Scenario: An invalid regexp is matched literally
- **WHEN** the lines are `a(b` and `ab`, and `a(` is typed
- **THEN** only `a(b` is listed

### Requirement: The selection stays near its line
When the input changes, the selected line SHALL stay selected if it still matches; otherwise the next matching line after it SHALL be selected, or the last match when none follows.

#### Scenario: The next match after point
- **WHEN** in `x1\nx2\nx3\nx4\n` with point on `x3`, `C-s` then `x` is typed
- **THEN** `x3` is still selected

#### Scenario: No match after it selects the last
- **WHEN** in `a\nb\nc\n` with point on `c`, `C-s` then `a` is typed
- **THEN** `a` is selected

### Requirement: Swiper previews the selected line
While swiper's minibuffer is open, the window it was opened from SHALL show the selected line with point at the end of the input's first match on it, scrolling to center that line only when it is out of view. With an empty input, point SHALL stay where it was on the starting line and go to the start of any other line.

#### Scenario: Moving the selection moves the preview
- **WHEN** in `one\ntwo\nthree\n` with point at the buffer start, `C-s tw` is typed
- **THEN** that window's point is after `tw` on line 2, and the minibuffer stays selected

#### Scenario: Quitting goes back
- **WHEN** a search previews another line and `C-g` is typed
- **THEN** the window's point and scroll are as before `C-s`

### Requirement: RET jumps to the match and saves the mark
`RET` SHALL move point to the end of the input's first match on the selected line, scrolling as the preview does. When point moved and no region is active, the mark SHALL be pushed where the search started, without activating it, and the echo area SHALL show `Mark saved where search started`.

#### Scenario: Search, move, jump
- **WHEN** in `alpha\nbeta\nalpine\n` with point at the start, `C-s al C-n RET` is typed
- **THEN** point is after `al` on line 3, the mark is at 0 and inactive, and the echo area shows `Mark saved where search started`

#### Scenario: An active region keeps its mark
- **WHEN** a region is active and `C-s` jumps to another line
- **THEN** the mark is where it was and still active

### Requirement: C-s repeats inside swiper
`C-s` in swiper's minibuffer SHALL select the next match, repeated by the prefix argument, or with an empty input SHALL take the previous search from history as `M-p` does.

#### Scenario: Next match
- **WHEN** at swiper's prompt with input `a` and `a1` selected among `a1`, `a2`, `C-s` is typed
- **THEN** `a2` is selected

#### Scenario: The last search
- **WHEN** a search for `beta` ended with `RET`, and `C-s C-s` is typed
- **THEN** the input is `beta`

### Requirement: Swiper's history keeps the searches
Swiper's history SHALL keep the typed input, not the selected line, whether the search ended with `RET` or `C-g`, and SHALL skip an empty input.

#### Scenario: A quit search is kept
- **WHEN** `C-s gam C-g` is typed, then `C-s M-p`
- **THEN** the input is `gam`

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
