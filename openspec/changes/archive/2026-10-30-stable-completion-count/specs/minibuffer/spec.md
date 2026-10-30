# Spec Delta

## MODIFIED Requirements

### Requirement: The frame display shows the completion list
While a completing minibuffer is open the frame display SHALL carry up to 10 candidates, a window of the matches that holds the selected one, and which of them is selected, and a count of the selected match's number and the number of matches, as ` 13/15 ` (`   0/0 ` with no match). The frontend SHALL draw the count before the prompt and the candidates one per line below the prompt line, the selected one highlighted.

#### Scenario: The list in the frame
- **WHEN** a command completing over 15 candidates is run and `C-u 12 C-n` is typed
- **THEN** the frame display's minibuffer lists 10 candidates ending with the 13th, which is selected, and its count is ` 13/15 `

## ADDED Requirements

### Requirement: The match count keeps its width while the minibuffer is open
The count SHALL be right-aligned in a field of at least 6 columns, followed by one space. While one minibuffer is open the field SHALL be as wide as the widest count it has shown, so the prompt, the text after it and the list stay in place as the count changes. A newly opened minibuffer SHALL start at 6 columns. A prompt without completion SHALL show no count.

#### Scenario: The count grows from 9 to 100 matches
- **WHEN** a completing minibuffer's matches go from 9 to 100
- **THEN** its count goes from `   1/9 ` to ` 1/100 ` and the prompt starts in the same column

#### Scenario: A count past the minimum field never narrows
- **WHEN** a completing minibuffer's count reaches `1/10000 ` and its matches fall to 9
- **THEN** its count is `    1/9 `, the same width

#### Scenario: The next minibuffer starts over
- **WHEN** that minibuffer closes and another completing minibuffer opens with 9 matches
- **THEN** its count is `   1/9 `
