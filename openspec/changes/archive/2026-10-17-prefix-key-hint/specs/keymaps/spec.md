# Spec Delta

## ADDED Requirements

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
