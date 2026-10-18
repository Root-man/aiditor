# Spec Delta

## Purpose

The minibuffer reads a string for a command: a one-line buffer with a prompt, edited with the normal editing keys, returned with `RET`, aborted with `C-g`, with a history per prompt.

## ADDED Requirements

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
Each returned string SHALL be added to the newest end of its prompt's history, unless it is empty or equal to the newest entry; a history SHALL keep at most 100. `M-p` SHALL replace the text with the next older entry and `M-n` with the next newer one, point at its end, each repeated by the prefix argument. `M-n` back past the newest SHALL restore the text typed before `M-p`.

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
