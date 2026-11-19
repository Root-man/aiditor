## ADDED Requirements

### Requirement: Terminal notification threshold
Settings SHALL accept a nonnegative integer `terminal_notify_after`, bundled as 10, the seconds a terminal command must run before its end notifies; 0 SHALL turn those notifications off. Invalid types or negative values SHALL reject settings through the existing error flow. Reloading SHALL apply it to commands ending afterwards.

#### Scenario: Defaults and overrides
- **GIVEN** a settings file omits `terminal_notify_after` or sets it to 30
- **WHEN** settings are loaded
- **THEN** the threshold is respectively 10 or 30

#### Scenario: Invalid threshold
- **GIVEN** valid settings are in effect
- **WHEN** settings with `terminal_notify_after = -1` or a string are loaded
- **THEN** the settings error names the key and the prior settings remain
