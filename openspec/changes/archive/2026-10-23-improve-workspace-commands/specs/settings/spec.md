# Spec Delta

## ADDED Requirements

### Requirement: Runtime configuration reload reports completion
After an accepted runtime configuration reload has applied settings, keymaps and project definitions without diagnostics, the editor SHALL show `Configuration reloaded` in the echo area and log it to `*Messages*`. The message SHALL follow the normal echo-area lifetime. Completion SHALL refer to configuration application, without waiting for background indexing or watch setup. A reload with read, parse, validation or key-binding diagnostics SHALL retain its diagnostic messages and existing per-component fallback behavior instead of showing the success message. Superseded, duplicate or otherwise discarded results SHALL NOT announce completion. Startup loading SHALL NOT announce runtime reload completion.

#### Scenario: Reload succeeds
- **GIVEN** valid configuration files and an editor running with earlier settings
- **WHEN** `C-x C-r` completes its accepted reload
- **THEN** the new configuration is in effect and `Configuration reloaded` appears in the echo area and once in `*Messages*`

#### Scenario: Reload through M-x
- **GIVEN** valid configuration files
- **WHEN** `M-x reload-settings` completes
- **THEN** the same completion message appears

#### Scenario: Invalid configuration retains useful diagnostics
- **GIVEN** a reload with invalid settings, a valid keymap and a valid project definition
- **WHEN** the accepted bundle is applied
- **THEN** the previous settings remain, the other valid components apply, and the echo area shows the settings diagnostic rather than a success message

#### Scenario: Key-binding diagnostics also prevent success confirmation
- **GIVEN** a syntactically valid keymap containing an invalid binding that produces a diagnostic
- **WHEN** the accepted reload is applied
- **THEN** the keymap diagnostic remains visible and the log records that diagnostic rather than `Configuration reloaded`

#### Scenario: Only the latest accepted reload announces completion
- **GIVEN** one reload is pending and a second reload has been requested
- **WHEN** the old result arrives, followed by the successful latest result and a duplicate of that result
- **THEN** the latest configuration is in effect and the completion message is logged exactly once

#### Scenario: A prior unrelated message is not a reload failure
- **GIVEN** another command has reported an error while a reload was pending
- **WHEN** the accepted reload applies without diagnostics
- **THEN** `Configuration reloaded` is shown and the earlier error remains available in `*Messages*`

#### Scenario: Completion message clears normally
- **GIVEN** the echo area shows `Configuration reloaded`
- **WHEN** the user presses a key or clicks
- **THEN** the message clears before that action is handled
