# Spec Delta

## ADDED Requirements

### Requirement: Project references in the prompt
Typing `@` in a session's editable prompt SHALL insert the literal `@` and open `Reference in <project>: ` with fuzzy completion over that project's indexed files and directories in every root. Candidates SHALL use the project finder's root labels, including duplicate-name suffixes; directories SHALL end in `/`, use the directory face and show `dir`, while files SHALL show their modification age. The session SHALL remain visible while choosing. First use SHALL start indexing when necessary and fill the open list when a compatible result arrives. The picker SHALL keep its captured root mapping for its lifetime.

#### Scenario: Every root and directory
- **GIVEN** a project with roots `/a/app` and `/b/app`, indexed files `src/main.rs` and `lib.rs`, and directories `src` and `empty`
- **WHEN** `@` is typed in the session prompt
- **THEN** `Reference in <project>: ` offers `app/src/main.rs`, `app/src/`, `app<2>/lib.rs` and `app<2>/empty/`, and the session stays visible

#### Scenario: First-use indexing
- **GIVEN** the project has no index
- **WHEN** `@` opens the picker and the index then answers
- **THEN** the open list gains the indexed files and directories without reopening

#### Scenario: Roots change during picking
- **GIVEN** the picker offers `app/src/main.rs` from `/old/app`
- **WHEN** the project's roots change to `/new/app` before that candidate is picked
- **THEN** the inserted chip still refers to `/old/app/src/main.rs`

#### Scenario: Candidate annotations
- **GIVEN** an indexed file with a known modification age and an indexed directory
- **WHEN** their candidates appear in the reference picker
- **THEN** the file shows its age and the directory shows `dir`

### Requirement: A reference replaces its triggering at sign
Picking a file or directory SHALL replace only the triggering `@` with its existing file, image or directory chip and place point after it. `RET` on a directory candidate SHALL attach it rather than enter it. `C-g` SHALL return to the session with the literal `@`, surrounding text and point preserved. An answer without a candidate SHALL require a match and leave the picker open. Input in the read-only transcript SHALL retain the read-only protection.

#### Scenario: Insert amid text
- **GIVEN** the prompt contains `compare  please` with point between its spaces
- **WHEN** `@` is typed and an indexed file is picked
- **THEN** the prompt contains `compare <file chip> please`, and sending includes the referenced file as a resource link

#### Scenario: Attach a directory
- **GIVEN** a directory candidate ending in `/` is selected
- **WHEN** `RET` is pressed
- **THEN** the prompt's `@` becomes a directory chip, sent as a directory resource link

#### Scenario: Cancel while output arrives
- **GIVEN** `@` opened the picker amid prompt text
- **WHEN** agent output arrives above the prompt and `C-g` is pressed
- **THEN** the prompt keeps the literal `@` and its surrounding text, with point after `@`

#### Scenario: Match required
- **GIVEN** the reference picker is open
- **WHEN** text matching no indexed candidate is submitted
- **THEN** the picker remains open and reports `Match required`

#### Scenario: Read-only transcript
- **GIVEN** point is in the read-only transcript
- **WHEN** `@` is typed
- **THEN** the editor reports `Text is read-only` and preserves the transcript
