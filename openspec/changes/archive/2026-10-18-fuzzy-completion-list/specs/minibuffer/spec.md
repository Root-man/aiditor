# Spec Delta

## ADDED Requirements

### Requirement: A prompt can complete over candidates
When a command declares candidates for its prompts, its minibuffer SHALL list the candidates that fuzzy-match the text, best match first, and every candidate in the given order while the text is empty. The list SHALL follow each change to the text. A prompt without candidates SHALL show no list.

#### Scenario: Typing filters candidates
- **WHEN** a command completing over `apple`, `banana` and `cherry` is run and `an` is typed
- **THEN** the list holds `banana` and not `apple` or `cherry`

#### Scenario: Empty text lists everything
- **WHEN** a command completing over `apple`, `banana` and `cherry` is run
- **THEN** the list is `apple`, `banana`, `cherry` and `apple` is selected

#### Scenario: A plain prompt has no list
- **WHEN** a command with a prompt and no candidates is run
- **THEN** the frame display's minibuffer has no candidates

### Requirement: C-n and C-p move the selection
`C-n` or `<down>` SHALL select the next candidate and `C-p` or `<up>` the previous one, each repeated by the prefix argument and stopping at the first and last candidate. Any change to the text SHALL select the first match again. These keys SHALL keep their global bindings in a minibuffer without candidates.

#### Scenario: Moving down and back
- **WHEN** at a prompt completing over `apple`, `banana`, `cherry` the keys `C-n C-n C-n` are typed, then `C-p`
- **THEN** `cherry` is selected, then `banana`

#### Scenario: Typing resets the selection
- **WHEN** at that prompt `C-n` then `a` is typed
- **THEN** the first match is selected

### Requirement: TAB completes and RET picks the selected candidate
`TAB` SHALL replace the minibuffer's text with the selected candidate, point at its end. `RET` SHALL return the selected candidate as the string, or the text as typed when no candidate matches.

#### Scenario: TAB inserts the selection
- **WHEN** at a prompt completing over `apple`, `banana`, `cherry` the keys `C-n TAB` are typed
- **THEN** the text is `banana` with point at its end

#### Scenario: RET returns the selection
- **WHEN** at that prompt `ch RET` is typed
- **THEN** the command gets the string `cherry`

#### Scenario: RET with no match returns the text
- **WHEN** at that prompt `zzz RET` is typed
- **THEN** the command gets the string `zzz`

### Requirement: The frame display shows the completion list
While a completing minibuffer is open the frame display SHALL carry up to 10 candidates, a window of the matches that holds the selected one, and which of them is selected. The frontend SHALL draw them one per line below the prompt line, the selected one highlighted.

#### Scenario: The list in the frame
- **WHEN** a command completing over 15 candidates is run and `C-u 12 C-n` is typed
- **THEN** the frame display's minibuffer lists 10 candidates ending with the 13th, which is selected
