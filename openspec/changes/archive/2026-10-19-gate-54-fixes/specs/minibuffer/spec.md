# Spec Delta

## MODIFIED Requirements

### Requirement: The frame display shows the completion list
While a completing minibuffer is open the frame display SHALL carry up to 10 candidates, a window of the matches that holds the selected one, and which of them is selected, and a count of the selected match's number and the number of matches, as `13/15 ` (`0/0 ` with no match). The frontend SHALL draw the count before the prompt and the candidates one per line below the prompt line, the selected one highlighted.

#### Scenario: The list in the frame
- **WHEN** a command completing over 15 candidates is run and `C-u 12 C-n` is typed
- **THEN** the frame display's minibuffer lists 10 candidates ending with the 13th, which is selected, and its count is `13/15 `
