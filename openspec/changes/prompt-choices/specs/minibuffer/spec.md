## MODIFIED Requirements

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

## ADDED Requirements

### Requirement: A command can ask with its own candidates
A command SHALL be able to ask a further question in the minibuffer with its own candidates, independent of the prompts it declares. That prompt SHALL complete over exactly those candidates, in the given order, instead of any candidates the command declares, and its answer SHALL be the command's next string as for any prompt.

#### Scenario: Each question lists its own candidates
- **WHEN** a command asks `First? ` with `a`, `b`, then on that answer asks `Second? ` with `x`, `y`, `z`
- **THEN** the first prompt lists `a`, `b` and the second lists `x`, `y`, `z`

#### Scenario: RET alone picks the first choice
- **WHEN** a command asks a question with `no`, `yes` and `RET` is typed
- **THEN** the command gets the string `no`
