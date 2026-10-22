## ADDED Requirements

### Requirement: Watched file limit configuration
The bundled `max_watched_files` SHALL be 10000. Settings and project files SHALL accept a nonnegative integer, with a project's explicit value overriding settings and omission inheriting settings. Zero SHALL allow no root watches. Invalid types and negative values SHALL reject the configuration through its existing error flow. Existing project commands SHALL preserve the override and its comments.

#### Scenario: Project override
- **GIVEN** settings limit one and a project override two with two kept files and one directory
- **WHEN** the project opens
- **THEN** it is watched using limit two

#### Scenario: Settings reload
- **GIVEN** a project inherits the global limit and is refused
- **WHEN** settings reload raises the limit enough for its counts
- **THEN** its roots become watched
