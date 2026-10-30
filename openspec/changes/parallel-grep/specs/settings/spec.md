## ADDED Requirements

### Requirement: Grep worker configuration
The bundled `grep_workers` SHALL be 1. Settings SHALL accept a positive integer; zero, negative and non-integer values SHALL reject the settings file through the existing error flow, keeping the last valid settings, or the bundled defaults at startup. A project grep SHALL use the count in effect when it starts, so a successful reload applies to the next search and a running search keeps its count.

#### Scenario: Default
- **WHEN** `settings.toml` does not set `grep_workers`
- **THEN** project grep searches with one worker

#### Scenario: Invalid count
- **WHEN** `settings.toml` sets `grep_workers = 0`, `-2` or `"many"`
- **THEN** the settings error names `grep_workers` and the previous settings stay

#### Scenario: Reload applies to the next search
- **GIVEN** a search started with `grep_workers = 1`
- **WHEN** `grep_workers = 4` is reloaded and a new search starts
- **THEN** the new search carries 4 workers
