# Spec Delta

## ADDED Requirements

### Requirement: Table width cap
Settings SHALL accept a nonnegative integer `table_width`, bundled as 100, the widest a session buffer draws a table, in chars. Invalid types or negative values SHALL reject settings through the existing error flow. Reloading SHALL apply the cap to tables drawn afterwards.

#### Scenario: Defaults and overrides
- **GIVEN** a settings file omits `table_width` or sets it to 60
- **WHEN** settings are loaded
- **THEN** the cap is respectively 100 or 60

#### Scenario: Invalid cap
- **GIVEN** valid settings are in effect
- **WHEN** settings with `table_width = -1` or a string are loaded
- **THEN** the settings error names the key and the prior settings remain
