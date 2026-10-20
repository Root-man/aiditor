# minibuffer Specification

## Purpose
The minibuffer reads a string for a command: a one-line buffer with a prompt, edited with the normal editing keys, returned with `RET`, aborted with `C-g`, with a history per prompt.

## Requirements

### Requirement: A prompting command reads its string in the minibuffer
When a command needs a prompted string the call did not give, the minibuffer SHALL open empty with that prompt and be selected, and the command SHALL NOT run yet. `RET` SHALL close it and run the command with its text as the next string and the prefix argument it was first called with; a further missing string SHALL open the minibuffer again.

#### Scenario: A key script answers a prompt
- **WHEN** a command with the prompt `Text: ` is run, then `hello RET` is typed
- **THEN** the command runs once with the string `hello`, and the original buffer is current and unchanged

#### Scenario: Two prompts ask in turn
- **WHEN** a command with two prompts is run and `a RET b RET` is typed
- **THEN** the command runs once with the strings `a` and `b`

#### Scenario: The prefix argument is kept
- **WHEN** `C-u 3` then a prompting command's key, then `x RET` are typed
- **THEN** the command runs with the prefix argument 3

### Requirement: C-g aborts the minibuffer
`C-g` in the minibuffer SHALL close it without running the command and show `Quit`.

#### Scenario: Abort
- **WHEN** a prompting command is run, `abc C-g` is typed
- **THEN** the command does not run, the original buffer is current and the echo area shows `Quit`

### Requirement: Normal editing works in the minibuffer
Keys the minibuffer's keymap does not bind SHALL run what the global keymap binds, on the minibuffer's text.

#### Scenario: Editing before returning
- **WHEN** at a prompt `helo <left> l C-a x DEL C-e RET` is typed
- **THEN** the command gets the string `hello`

### Requirement: Each prompt keeps a history
Each returned string SHALL be added to the newest end of its prompt's history, unless it is empty or equal to the newest entry; a history SHALL keep at most 100. An answer to a question a command asks with its own choices SHALL NOT be added to any history. `M-p` SHALL replace the text with the next older entry and `M-n` with the next newer one, point at its end, each repeated by the prefix argument. `M-n` back past the newest SHALL restore the text typed before `M-p`.

#### Scenario: Earlier answers come back
- **WHEN** a prompt was answered `one`, then `two`, and at the same prompt `M-p` is typed, then `M-p` again
- **THEN** the text is `two`, then `one`

#### Scenario: Back to what was typed
- **WHEN** at a prompt with history `x` is typed, then `M-p M-n`
- **THEN** the text is `x` again

#### Scenario: The ends of the history
- **WHEN** at a prompt with one history entry `M-p M-p` is typed, or `M-n` with no `M-p` before it
- **THEN** the echo area shows `Beginning of history; no preceding item` or `End of history; no default available` and the text is unchanged

#### Scenario: Prompts keep separate histories
- **WHEN** a prompt `A: ` was answered `a` and another prompt `B: ` is open
- **THEN** `M-p` reports the beginning of history

#### Scenario: Confirmations keep no history
- **WHEN** with a modified file buffer `C-x C-c` is typed, `Save file <path>? ` is answered `no` and `Modified buffers exist; exit anyway? ` is answered `no`, then `C-x C-c` asks `Save file <path>? ` again and `M-p` is typed
- **THEN** the echo area shows `Beginning of history; no preceding item`

### Requirement: The minibuffer stays put while active
A command that prompts while the minibuffer is active SHALL fail with `Command attempted to use minibuffer while in minibuffer`. While the minibuffer is selected, splitting, selecting another window and deleting windows SHALL fail and leave the windows as they were. A click in a window SHALL abort the minibuffer, then act as a click.

#### Scenario: Split from the minibuffer
- **WHEN** `C-x 2` is typed at a prompt
- **THEN** the echo area shows an error and the frame still has one window

#### Scenario: A click aborts
- **WHEN** a window is clicked at a prompt
- **THEN** the minibuffer is closed, the command did not run and point moved to the click

### Requirement: The frame display shows the minibuffer
While the minibuffer is active the frame display SHALL carry its prompt and the line holding point with the cursor, and the frontend SHALL draw them in the echo area line, any message after them in brackets.

#### Scenario: Prompt and text in the frame
- **WHEN** a command with the prompt `Text: ` is run and `hi` is typed
- **THEN** the frame display's minibuffer has the prompt `Text: `, the text `hi` and the cursor at column 2

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
While a completing minibuffer is open the frame display SHALL carry up to 10 candidates, a window of the matches that holds the selected one, and which of them is selected, and a count of the selected match's number and the number of matches, as `13/15 ` (`0/0 ` with no match). The frontend SHALL draw the count before the prompt and the candidates one per line below the prompt line, the selected one highlighted.

#### Scenario: The list in the frame
- **WHEN** a command completing over 15 candidates is run and `C-u 12 C-n` is typed
- **THEN** the frame display's minibuffer lists 10 candidates ending with the 13th, which is selected, and its count is `13/15 `

### Requirement: The completion list only grows while the minibuffer is open
While one minibuffer is open, the frame display SHALL carry as many completion rows as the most it has shown since it opened, and the frontend SHALL draw the rows past the shown matches empty, so the windows above keep their height. A newly opened minibuffer SHALL start at the rows its own matches need.

#### Scenario: Fewer matches keep the height
- **WHEN** `find-file` lists a directory with 10 entries and the text then moves to a directory with 5
- **THEN** the frame display's minibuffer lists 5 candidates in 10 rows, and the frontend's window area keeps its height

#### Scenario: The next minibuffer starts afresh
- **WHEN** a minibuffer that showed 10 rows closes and a prompt with 3 candidates opens
- **THEN** the frame display's minibuffer carries 3 rows

### Requirement: Candidates can carry annotations
A command that completes over candidates SHALL be able to annotate each one with text worked out when the minibuffer opens. The frame display SHALL carry each shown candidate's annotation, and the frontend SHALL draw it after the candidate. Annotations SHALL NOT be matched against the text.

#### Scenario: Annotations are not matched
- **WHEN** a command completes over `apple` annotated `zz` and `banana` annotated `none`, and `zz` is typed
- **THEN** the list is empty

#### Scenario: Annotations in the frame
- **WHEN** that command is run
- **THEN** the frame display's minibuffer lists `apple` with the annotation `zz` and `banana` with `none`

### Requirement: M-RET returns the text as typed
`M-RET` in a completing minibuffer SHALL close it and return its text as typed, whether or not a candidate matches.

#### Scenario: A name that matches something else
- **WHEN** at a prompt completing over `apple`, `banana`, `cherry` the keys `ch M-RET` are typed
- **THEN** the command gets the string `ch`

### Requirement: A command can ask with its own candidates
A command SHALL be able to ask a further question in the minibuffer with its own candidates, independent of the prompts it declares. That prompt SHALL complete over exactly those candidates, in the given order, instead of any candidates the command declares, and its answer SHALL be the command's next string as for any prompt.

#### Scenario: Each question lists its own candidates
- **WHEN** a command asks `First? ` with `a`, `b`, then on that answer asks `Second? ` with `x`, `y`, `z`
- **THEN** the first prompt lists `a`, `b` and the second lists `x`, `y`, `z`

#### Scenario: RET alone picks the first choice
- **WHEN** a command asks a question with `no`, `yes` and `RET` is typed
- **THEN** the command gets the string `no`
