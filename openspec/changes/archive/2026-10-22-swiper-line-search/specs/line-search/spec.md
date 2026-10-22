# Spec Delta

## Purpose

Swiper: searching the current buffer by listing its matching lines in the minibuffer, previewing the selected one and jumping to it, as ivy's swiper does.

## ADDED Requirements

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
