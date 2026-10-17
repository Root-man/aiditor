## ADDED Requirements

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
