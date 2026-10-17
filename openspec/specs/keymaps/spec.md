# keymaps Specification

## Purpose
Resolves key sequences to commands through layered keymaps with prefix keys, so modes can add or shadow bindings and the user hears about keys that do nothing.

## Requirements

### Requirement: Key sequences resolve through prefix keys
A keymap SHALL bind a key sequence either to a command or as a prefix. After a prefix key the core SHALL wait for the next key and look up the longer sequence; when a sequence is bound to a command, that command SHALL run and the pending sequence SHALL be cleared.

#### Scenario: Two-level prefix
- **WHEN** a test binds `C-x 4 f` to a command and presses `C-x 4 f`
- **THEN** the command runs once and the next key starts a new sequence

#### Scenario: Existing bindings still work
- **WHEN** `C-x C-s` is pressed in a buffer visiting a file
- **THEN** the buffer is saved, as before

### Requirement: Keymaps are layered
The core SHALL look up the pending sequence in the transient map, then each minor-mode map of the current buffer in order, then its major-mode map, then the global map. The first map that binds the sequence, as a command or a prefix, SHALL decide it.

#### Scenario: A minor mode shadows a global binding
- **WHEN** the global map binds `C-c a` to one command, a minor mode of the current buffer binds it to another, and `C-c a` is pressed
- **THEN** the minor mode's command runs

#### Scenario: Removing the mode restores the global binding
- **WHEN** that minor mode is removed from the buffer and `C-c a` is pressed again
- **THEN** the global map's command runs

#### Scenario: A mode prefix does not hide other global keys under it
- **WHEN** a minor mode binds `C-x a` and `C-x C-s` is pressed
- **THEN** the global `C-x C-s` runs

#### Scenario: The major mode sits below minor modes and above global
- **WHEN** the global map, the major mode and a minor mode all bind one key, and the minor mode is then removed
- **THEN** the minor mode's command runs first, then the major mode's

#### Scenario: The transient map wins once
- **WHEN** a transient map binds a key that the global map also binds, and the key is pressed twice
- **THEN** the transient command runs the first time and the global one the second time

### Requirement: Undefined keys are reported
A sequence that no layer binds SHALL be dropped and the message line SHALL show it in `kbd` form followed by `is undefined`. A single key with neither Ctrl nor Meta that no layer binds SHALL instead keep its editing behavior (typing, `RET`, `DEL`, arrows); unbound `TAB` and `ESC` have none and are reported.

#### Scenario: Undefined after a prefix
- **WHEN** `C-x C-q` is pressed and nothing binds it
- **THEN** the buffer is unchanged and the message line shows `C-x C-q is undefined`

#### Scenario: Undefined modified key
- **WHEN** `M-z` is pressed and nothing binds it
- **THEN** the message line shows `M-z is undefined`

#### Scenario: Unbound TAB
- **WHEN** `TAB` is pressed and nothing binds it
- **THEN** the message line shows `TAB is undefined`

#### Scenario: Plain keys still type
- **WHEN** `ab` is typed
- **THEN** the text is `ab` and no message is shown

### Requirement: C-g quits
`C-g` after a prefix key SHALL cancel the pending sequence without running a command, and the message line SHALL show `Quit`. `C-g` with no pending prefix SHALL run the `keyboard-quit` command, which deactivates the mark and shows `Quit`.

#### Scenario: Cancel a prefix
- **WHEN** `C-x C-g` is pressed and then `C-s`
- **THEN** nothing is saved, the message line showed `Quit`, and `C-s` is looked up on its own

#### Scenario: C-g on its own
- **WHEN** `C-g` is pressed with no prefix pending
- **THEN** the message line shows `Quit`

#### Scenario: C-g clears the region
- **WHEN** `C-SPC C-f C-f` is pressed in `hello` and then `C-g`
- **THEN** the mark is still at 0, inactive, and the display shows no region

### Requirement: Default bindings come from a bundled keymap file
The editor SHALL ship a bundled keymap file holding one table per keymap name (`global` or a mode name), each mapping `kbd` key sequences to command names, and SHALL build its keymaps from it.

#### Scenario: Bundled defaults bind the existing keys
- **WHEN** no user keymap file exists and `C-x C-s` is pressed in a buffer visiting a file
- **THEN** the buffer is saved and no error is shown

### Requirement: The user keymap file overrides the bundled one
The editor SHALL read `keymap.toml` from the user's config dir and bind its entries after the bundled ones, so each user entry replaces whatever the bundled file bound at that sequence and every other bundled binding stays.

#### Scenario: User rebinds a key
- **WHEN** the user file binds `C-x u` to `undo-redo`
- **THEN** `C-x u` redoes and `C-/` still undoes

#### Scenario: User adds a mode binding
- **WHEN** the user file has a `[test-mode]` table binding `C-c a` and the current buffer has that minor mode
- **THEN** `C-c a` runs the command it names

### Requirement: Bad keymap entries are reported and skipped
An entry whose key sequence does not parse, whose command does not exist, or whose value is not a command name, and a top-level entry that is not a table, SHALL be skipped. The message line SHALL show `keymap:` followed by every skipped entry and why, and all other entries SHALL load.

#### Scenario: One bad entry among good ones
- **WHEN** the user file binds `C-c a` to `undo` and `C-c b` to `no-such-command`
- **THEN** `C-c a` undoes and the message line names `C-c b` and `no-such-command`

#### Scenario: Unparseable key
- **WHEN** the user file binds `C-xy` to `undo`
- **THEN** the message line names `C-xy` and the other bindings work

### Requirement: An unreadable keymap file keeps the keymaps in effect
A user keymap file that cannot be read or is not valid TOML SHALL be reported on the message line as a `keymap:` error, and the keymaps in effect before SHALL stay.

#### Scenario: Invalid TOML on reload
- **WHEN** the user file binds `C-c a` and is loaded, then is changed to invalid TOML and reloaded
- **THEN** a `keymap:` error is shown and `C-c a` still runs its command

### Requirement: C-x C-r reloads the keymaps
`C-x C-r` SHALL read the user keymap file again and rebuild the keymaps from the bundled file and it, without a restart.

#### Scenario: A changed binding takes effect
- **WHEN** the user file binds `C-c a` to one command, is loaded, then is changed to bind it to another and `C-x C-r` is pressed
- **THEN** `C-c a` runs the new command

#### Scenario: A removed binding reverts to the default
- **WHEN** the user file rebinds `C-x u`, is loaded, then is deleted and `C-x C-r` is pressed
- **THEN** `C-x u` undoes again

### Requirement: A hint follows a pending prefix
When a key leaves a prefix pending and no other key arrives for 1 s, the display SHALL show a hint: the pending keys in `kbd` form and, sorted by key, every key that can follow them with the command it runs, or `+prefix` when it leads to a further prefix. Each key SHALL show the binding the layered lookup would use. The next key, `C-g` included, SHALL hide the hint.

#### Scenario: C-x lists its bindings after the delay
- **WHEN** `C-x` is pressed and 1 s passes
- **THEN** the display's hint lists `C-r reload-settings`, `C-s save-buffer` and `u undo` under `C-x`

#### Scenario: No hint before the delay
- **WHEN** `C-x` is pressed and less than 1 s passes
- **THEN** the display shows no hint

#### Scenario: A further prefix is marked
- **WHEN** a test binds `C-x 4 f`, presses `C-x` and waits 1 s
- **THEN** the hint shows `4` as `+prefix`

#### Scenario: A mode binding shadows the global one in the hint
- **WHEN** a minor mode of the current buffer binds `C-x u` to another command, `C-x` is pressed and 1 s passes
- **THEN** the hint shows the minor mode's command for `u` and still lists the global `C-s`

#### Scenario: The next key hides the hint
- **WHEN** the hint for `C-x` is shown and `C-g` is pressed
- **THEN** the display shows no hint, and pressing a further key and waiting starts no hint
