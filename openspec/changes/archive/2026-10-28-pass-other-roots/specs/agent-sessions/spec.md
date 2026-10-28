# Spec Delta

## ADDED Requirements

### Requirement: Sessions start with the project's other roots
Starting or loading a session SHALL pass every root of the project but the primary, as the project lists them when the session starts or loads. Roots added or removed while it runs SHALL apply from its next start or load. An agent that can't take them SHALL get `<agent> can't take additional directories` noted in `*Messages*`, and the session SHALL start without them.

#### Scenario: Other roots are passed
- **WHEN** `agent-start` runs in a project with primary `app` and roots `lib` and `docs`
- **THEN** the spawn carries `lib` and `docs`

#### Scenario: The agent can't take them
- **WHEN** the agent service reports that `claude` can't take additional directories
- **THEN** `*Messages*` holds `claude can't take additional directories`

### Requirement: The roots preamble
A new session's first prompt SHALL carry, before the user's text and as its own text block, a preamble listing each root's path with its role line, the primary first and marked as primary. The preamble SHALL be built from the project when the session starts. It SHALL NOT be shown in the session buffer, SHALL NOT be sent on later prompts or by a loaded session, and SHALL be skipped when the project has no other roots and the primary has no role line.

#### Scenario: The first prompt carries the preamble
- **WHEN** the first prompt `hi` is sent in a new session of a project with primary `app` (role `The app`) and root `lib` (no role)
- **THEN** the request's blocks are the text `Project roots:\n- <app> (primary): The app\n- <lib>` and then `hi`, and the session buffer shows only `hi`

#### Scenario: Later prompts carry none
- **WHEN** a second prompt is sent in that session
- **THEN** its request holds only the user's text

#### Scenario: A loaded session carries none
- **WHEN** the first prompt is sent in a loaded session
- **THEN** its request holds only the user's text

#### Scenario: Nothing to list
- **WHEN** the first prompt is sent in a project with no other roots and no role line on the primary
- **THEN** its request holds only the user's text
