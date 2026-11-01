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
A command that prompts while the minibuffer is active SHALL fail with `Command attempted to use minibuffer while in minibuffer`. While the minibuffer is selected, splitting, selecting another window and deleting windows SHALL fail and leave the windows as they were.

#### Scenario: Split from the minibuffer
- **WHEN** `C-x 2` is typed at a prompt
- **THEN** the echo area shows an error and the frame still has one window

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
`C-n` or `<down>` SHALL select the next candidate and `C-p` or `<up>` the previous one, each repeated by the prefix argument and stopping at the first and last candidate. Any change to the text SHALL select the first match again, except find-file with an empty filename query and project-find-file with an empty query SHALL clear the selection. In either file finder, C-n from no selection SHALL select the first candidate and C-p the last. These keys SHALL keep their global bindings in a minibuffer without candidates.

#### Scenario: Moving down and back
- **GIVEN** a prompt completes over `apple`, `banana`, `cherry`
- **WHEN** `C-n C-n C-n` is typed, then `C-p`
- **THEN** `cherry` is selected, then `banana`

#### Scenario: Typing resets the selection
- **GIVEN** a prompt completes over `apple`, `banana`, `cherry`
- **WHEN** `C-n` then `a` is typed
- **THEN** the first match is selected

### Requirement: TAB completes and RET picks the selected candidate
`TAB` SHALL replace the minibuffer's text with the selected candidate, point at its end. `RET` SHALL return the selected candidate as the string, or the text as typed when no candidate is selected. In either file finder, TAB with no selection SHALL complete the first match when one exists.

#### Scenario: TAB inserts the selection
- **GIVEN** a prompt completes over `apple`, `banana`, `cherry`
- **WHEN** `C-n TAB` is typed
- **THEN** the text is `banana` with point at its end

#### Scenario: RET returns the selection
- **GIVEN** a prompt completes over `apple`, `banana`, `cherry`
- **WHEN** `ch RET` is typed
- **THEN** the command gets the string `cherry`

#### Scenario: RET with no match returns the text
- **GIVEN** a prompt completes over `apple`, `banana`, `cherry`
- **WHEN** `zzz RET` is typed
- **THEN** the command gets the string `zzz`

### Requirement: The frame display shows the completion list
While a completing minibuffer is open the frame display SHALL carry up to 10 candidates, a window of the matches that holds the selected one, and which of them is selected, and a count of the selected match's number and the number of matches, as ` 13/15 ` (`   0/0 ` with no match in ordinary completion, `   */N ` with no selection in either file finder). The frontend SHALL draw the count before the prompt and the candidates one per line below the prompt line, the selected one highlighted.

#### Scenario: The list in the frame
- **GIVEN** a prompt completes over 15 candidates
- **WHEN** `C-u 12 C-n` is typed
- **THEN** the frame display's minibuffer lists 10 candidates ending with the 13th, which is selected, and its count is ` 13/15 `

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

### Requirement: A completing prompt can preview its selection
When a command declares a preview for its prompts, the window the minibuffer was opened from SHALL show the buffer the preview gives for the selected candidate, following every change of the selection, from the moment the list opens. A candidate the preview gives no buffer for, or no selection, SHALL show that window as it was. Previewing SHALL NOT make a buffer the most recent one. A question a command asks with its own choices SHALL NOT preview, unless the command declares no prompts, so that question is its only one.

#### Scenario: Moving the selection previews
- **WHEN** a previewing prompt is opened from a window showing `a` and `C-n` selects a candidate whose buffer is `b`
- **THEN** that window shows `b`, and the minibuffer stays selected

#### Scenario: No buffer shows the window as it was
- **WHEN** the text matches no candidate
- **THEN** the window shows `a` again, with its old point

### Requirement: Closing the minibuffer restores the previewed window
Closing a previewing minibuffer in any way (`C-g`, `RET`, switching workspace) SHALL first put the window it was opened from back exactly as it was: its buffer, point, mark, whether the mark is active, mark ring and scroll. `RET` SHALL then run the command as it would without a preview.

#### Scenario: Preview then C-g
- **WHEN** a window has point, an active mark and a mark ring, a previewing prompt previews other buffers, and `C-g` is typed
- **THEN** the frame's windows are exactly as before the prompt opened

#### Scenario: RET runs the command after restoring
- **WHEN** a previewing prompt previews a candidate and `RET` is typed
- **THEN** the command runs with the candidate, from the window as it was before the prompt

### Requirement: A preview can show a file not yet visited
A preview SHALL be able to name a file. A file visited in the current workspace SHALL show its buffer. Any other file SHALL be read and prepared as text off the core thread into a hidden, read-only preview buffer, the window keeping what it showed until the read answers. Only the current preview request of the same prompt session SHALL be allowed to install its result; a reply from a closed prompt or an obsolete selection SHALL be dropped, even if a later prompt selects the same path. A file that is missing, binary, unreadable, not a regular file, or larger than 10 MiB SHALL show the window as it was, and a device or pipe SHALL NOT be read.

#### Scenario: Previewing an unvisited file
- **GIVEN** a previewing prompt is open
- **WHEN** it selects a file no buffer visits
- **THEN** once it is read, the window shows its text, and no buffer list offers the preview buffer

#### Scenario: A late read
- **GIVEN** a selected file is being read for preview
- **WHEN** the selection moves on before the read answers
- **THEN** that answer is dropped and the window displays only an eligible current preview

#### Scenario: The same path in a later prompt
- **GIVEN** a preview read belongs to a prompt that has closed
- **WHEN** a new prompt selects the same path before the old read answers
- **THEN** the new prompt uses its own current request and the old result leaves its window unchanged

#### Scenario: Selecting a device
- **GIVEN** `C-x C-f` lists `/dev/`
- **WHEN** `zero` is selected
- **THEN** no bytes are read from it and the window shows what it showed before the prompt

#### Scenario: Selecting a named pipe
- **GIVEN** a previewing prompt lists a named pipe and a text file
- **WHEN** the pipe is selected, then the text file
- **THEN** the window shows the text file's text

#### Scenario: A file over the limit
- **WHEN** a previewing prompt selects a file larger than 10 MiB that no buffer visits
- **THEN** the file is not read for the preview, the window shows what it showed before and `RET` still visits it

### Requirement: One preview buffer at a time
There SHALL be at most one preview buffer. It SHALL be killed when another file is read for preview and when the minibuffer closes, so `RET` visits the file as it would without a preview.

#### Scenario: Closing kills the preview buffer
- **WHEN** a file is previewed and `C-g` is typed
- **THEN** the window is as it was and no buffer is left for the file

### Requirement: A command can filter its own candidates
A command's spec SHALL be able to filter its candidates itself in place of the fuzzy matcher, given the text and every candidate, returning those that match in the order it chooses. When the text changes, the selected candidate SHALL stay selected if it still matches, else the next match after it in the given order SHALL be, else the last match. `RET` with nothing matching SHALL show `Match required` and keep the minibuffer open.

#### Scenario: Matches keep the filter's order
- **WHEN** a filtering prompt lists `b1`, `a`, `b2` and `b` is typed
- **THEN** the list shows `b1` then `b2`

#### Scenario: A match is required
- **WHEN** at a filtering prompt the text matches nothing and `RET` is typed
- **THEN** the echo area shows `Match required`, the minibuffer stays open and the command does not run

### Requirement: A command can preselect a candidate
A command's spec SHALL be able to name, given the args so far, which candidate is selected when its list opens.

#### Scenario: The preselected candidate
- **WHEN** a prompt over `a`, `b`, `c` preselects the second
- **THEN** `b` is selected when the list opens, and the count reads `2/3`

### Requirement: A preview can show a position in the window's buffer
A preview SHALL be able to name a position in the buffer of the window the minibuffer was opened from. That window SHALL show its own buffer with point there, keeping its scroll from one such preview to the next and centering point's line only when it is out of view. Closing the minibuffer SHALL restore the window as for any preview.

#### Scenario: A position preview keeps the scroll while in view
- **WHEN** one position preview scrolls the window to center line 100, and the next names line 102
- **THEN** the window shows line 102 without scrolling

### Requirement: A command's keymap applies in its minibuffer
A minibuffer SHALL have the name of the command it reads for as a minor mode, so a keymap named after the command binds keys at its prompts.

#### Scenario: A command's key at its prompt
- **WHEN** the keymap `swiper` binds `C-s` and swiper's prompt is open
- **THEN** `C-s` runs that binding, not the global one

### Requirement: A filtering prompt's history keeps the typed text
A filtering prompt SHALL add the typed text to its history instead of the selected candidate, both when it returns with `RET` and when it is aborted with `C-g`, following the history's rules for empty and repeated entries.

#### Scenario: The typed text is kept
- **WHEN** at a filtering prompt `b` is typed, `b1` is selected and `RET` is typed, and the prompt is opened again and `M-p` typed
- **THEN** the text is `b`

### Requirement: Preview requests coalesce to the current selection
The editor SHALL keep at most one preview file read active and one latest pending request, including across prompt closures and reopenings. A selection change SHALL replace an obsolete pending request. Closing a prompt SHALL cancel its queued request and invalidate its active result. Preparation of preview text SHALL leave the editor able to process input. Existing preview restoration, visited-buffer reuse, full-file display of files within the preview limit, and normal file visitation on `RET` SHALL remain in effect.

#### Scenario: Moving quickly through candidates
- **GIVEN** file `a` is being read and file `b` is queued for preview
- **WHEN** the selection advances to file `c` before `a` finishes
- **THEN** the pending request becomes `c`, and `c` is read after `a` finishes

#### Scenario: Input during preview preparation
- **GIVEN** a large file is being prepared for preview
- **WHEN** the user types another query or presses `C-g`
- **THEN** the input is handled before preview preparation completes

#### Scenario: Accepting after a preview
- **GIVEN** a hidden preview shows a selected file
- **WHEN** the user presses `RET`
- **THEN** the origin is restored, the hidden preview is released, and the ordinary visit reads the selected file according to file-visiting rules

### Requirement: Fuzzy completion lists highlight matched characters
Every fuzzy completing prompt SHALL draw the matched characters of each visible candidate in the theme's `completion-match` face. Highlights SHALL reflect the pattern that produced the displayed matches, including smart case, normalization and multi-atom patterns, and follow query changes and scrolling. Unicode matches SHALL highlight complete grapheme clusters. An empty pattern SHALL show candidates in their normal face. Selected rows SHALL retain their selection background, and annotations SHALL retain their dimmed appearance separately from candidate highlights. Swiper and project grep retain their existing filtering and rendering.

#### Scenario: Command initials
- **GIVEN** the `M-x` prompt is open
- **WHEN** `pff` is typed
- **THEN** the `p`, first `f` and second `f` in `project-find-file` use `completion-match`

#### Scenario: Other fuzzy prompts
- **GIVEN** `project-find-file`, `find-file` or `C-x b` is completing
- **WHEN** a query matches a candidate
- **THEN** its matched characters use `completion-match`, including on the selected row, and annotations stay dimmed

#### Scenario: Unicode and multiple atoms
- **GIVEN** candidates contain accented text, decomposed accents or emoji
- **WHEN** a multi-atom fuzzy query matches them
- **THEN** the corresponding complete grapheme clusters are highlighted and the candidate text stays intact

#### Scenario: Updating the visible matches
- **GIVEN** a fuzzy completion list has more than ten matching candidates
- **WHEN** the query changes, the selection scrolls the list or the query is cleared
- **THEN** each visible candidate's highlights reflect its displayed match pattern, with normal faces for the cleared query

### Requirement: Filesystem completion uses directory faces and aligned annotations
Filesystem completion SHALL draw directory names in the theme's `directory` face and ordinary filenames in the default face, with matched characters still in `completion-match`. Metadata SHALL use the `annotation` face in right-aligned mode, size and age columns ending at the frame's right inset. It SHALL never participate in fuzzy matching or match highlighting. Long names SHALL truncate before annotations rather than displacing their columns; in a narrow frame annotations SHALL remain clipped to the row.

#### Scenario: Directory and file faces
- **GIVEN** find-file lists `sub/` and `a.txt`
- **WHEN** the list is drawn
- **THEN** `sub/` uses the directory face and `a.txt` the default face, with any query highlights layered over those faces

#### Scenario: Dimmed metadata is outside matching
- **GIVEN** a file's annotation contains `41 B` and its name does not match `41`
- **WHEN** `41` is typed
- **THEN** that file is omitted from matches

#### Scenario: Long filenames
- **GIVEN** filesystem entries with long names and metadata
- **WHEN** the frame narrows
- **THEN** names are clipped before the dimmed columns and the annotations stay within the row

#### Scenario: Project file annotations
- **GIVEN** project-find-file displays indexed paths with metadata
- **WHEN** the list is drawn or narrowed
- **THEN** it uses the same dimmed right-aligned metadata columns and name clipping as find-file
