## ADDED Requirements

### Requirement: Keys can be bound to a named prefix keymap
A keymap file entry whose value names another table, and no command, SHALL bind its sequence as a prefix holding that table's bindings, from every keymap file, with entries bound directly under that sequence laid over them. A named prefix SHALL work under every sequence bound to it, in any keymap, and may itself bind named prefixes. A value naming neither a command nor a table, and a prefix that contains itself, SHALL be reported as a bad entry.

#### Scenario: The user moves the project prefix
- **WHEN** the user file binds `C-c p` to `project-prefix` and `C-x p` to `false`
- **THEN** `C-c p f` runs `project-find-file`, every project command is under `C-c p`, and `C-x p f` is undefined

#### Scenario: One prefix under two sequences
- **WHEN** the user file binds `C-c p` to `project-prefix` and keeps `C-x p`
- **THEN** `C-c p f` and `C-x p f` both run `project-find-file`

#### Scenario: Adding to a prefix reaches every sequence
- **WHEN** the user file binds `C-c p` to `project-prefix` and adds `z` to `[project-prefix]`
- **THEN** both `C-c p z` and `C-x p z` run it

#### Scenario: Unknown or cyclic prefix
- **WHEN** the user file binds `C-c q` to `no-such-prefix`, and `[loop]` binds `l` to `loop`
- **THEN** the message line names `no-such-prefix` and the cycle, and the other entries load

### Requirement: An entry can unbind a key
A keymap file entry whose value is `false` SHALL remove whatever its keymap binds at that sequence, so lower layers or nothing apply there.

#### Scenario: Unbind a bundled key
- **WHEN** the user file binds `C-x u` to `false`
- **THEN** `C-x u` is undefined and `C-/` still undoes

### Requirement: Bundled prefixes are named
The bundled keymap SHALL hold the project commands in `project-prefix` on `C-x p`, the workspace commands in `workspace-prefix` on `C-x t`, and the agent commands in `agent-prefix` on `C-x a`: `a` agent-start, `l` agent-sessions, `f` agent-fork-session, `r` agent-rollback-session, `m` agent-select-model, `e` agent-select-effort.

#### Scenario: Agent prefix
- **WHEN** `C-x a` is pressed and 1 s passes
- **THEN** the hint lists `a agent-start` and `l agent-sessions`

## MODIFIED Requirements

### Requirement: A hint follows a pending prefix
When a key leaves a prefix pending and no other key arrives for 1 s, the display SHALL show a hint: the pending keys in `kbd` form and, sorted by key, every key that can follow them with the command it runs, or `+<name>` when it leads to a named prefix keymap and `+prefix` when it leads to an unnamed one. Each key SHALL show the binding the layered lookup would use. The next key, `C-g` included, SHALL hide the hint.

#### Scenario: C-x lists its bindings after the delay
- **WHEN** `C-x` is pressed and 1 s passes
- **THEN** the display's hint lists `C-r reload-settings`, `C-s save-buffer` and `u undo` under `C-x`

#### Scenario: No hint before the delay
- **WHEN** `C-x` is pressed and less than 1 s passes
- **THEN** the display shows no hint

#### Scenario: A further prefix is marked
- **WHEN** a test binds `C-x 4 f`, presses `C-x` and waits 1 s
- **THEN** the hint shows `4` as `+prefix`

#### Scenario: A named prefix shows its name
- **WHEN** `C-x` is pressed and 1 s passes
- **THEN** the hint shows `p` as `+project-prefix` and `t` as `+workspace-prefix`

#### Scenario: A mode binding shadows the global one in the hint
- **WHEN** a minor mode of the current buffer binds `C-x u` to another command, `C-x` is pressed and 1 s passes
- **THEN** the hint shows the minor mode's command for `u` and still lists the global `C-s`

#### Scenario: The next key hides the hint
- **WHEN** the hint for `C-x` is shown and `C-g` is pressed
- **THEN** the display shows no hint, and pressing a further key and waiting starts no hint
