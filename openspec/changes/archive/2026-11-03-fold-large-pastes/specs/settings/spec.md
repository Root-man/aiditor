## ADDED Requirements

### Requirement: Paste folding threshold
Settings SHALL accept a nonnegative integer `paste_fold_lines`, bundled as 20. Invalid types or negative values SHALL reject settings through the existing error flow. Reloading SHALL apply the threshold to subsequent prompt yanks.

#### Scenario: Defaults and overrides
- **GIVEN** a settings file omits the threshold or sets it to 3
- **WHEN** settings are loaded
- **THEN** the threshold is respectively 20 or 3

#### Scenario: Invalid threshold
- **GIVEN** valid settings are in effect
- **WHEN** settings with `paste_fold_lines = -1` or a string are loaded
- **THEN** the settings error names the key and the prior settings remain
