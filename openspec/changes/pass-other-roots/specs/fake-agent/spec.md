# Spec Delta

## ADDED Requirements

### Requirement: Additional directories
With `"directories": true` in its script, the fake SHALL advertise `sessionCapabilities.additionalDirectories`. On `session/new` and `session/load` it SHALL report `dirs=` with the request's `additionalDirectories` joined by `,` on stderr.

#### Scenario: Advertised and reported
- **WHEN** a script with `"directories": true` gets `session/new` with additional directories `b` and `c`
- **THEN** it advertised the capability and writes `dirs=b,c` to stderr

#### Scenario: Not advertised
- **WHEN** a script without `directories` is initialized
- **THEN** its capabilities have no `additionalDirectories`
