## MODIFIED Requirements

### Requirement: The mode line shows the current mode
A session buffer's mode line SHALL show the first config option of category `mode`, when it is a select, as `[<current value>]` on the left, following the model and effort and before the session title. It SHALL follow `session/new` or `session/load`, `config_option_update` and `session/set_config_option` answers, and omit the mode when no mode select exists or the session is not ready. The mode SHALL use `status-info` for `plan` and `read-only`, plain `mode-line` for `default`, `status-waiting` for `acceptEdits` and `workspace-write`, `status-running` for `auto` and `agent`, and `status-failed` for `bypassPermissions` and `agent-full-access`. Unknown modes SHALL use plain `mode-line`.

#### Scenario: The mode at start
- **GIVEN** a starting session
- **WHEN** it becomes ready with current mode `auto`
- **THEN** its mode line shows `[auto]` in `status-running`

#### Scenario: A mode change
- **GIVEN** a session in `auto`
- **WHEN** a config update or S-TAB's answer reports `plan`
- **THEN** its mode segment becomes `[plan]` in `status-info`

#### Scenario: Not ready
- **GIVEN** a session not yet ready or whose latest config has no mode select
- **WHEN** its mode line is displayed
- **THEN** the modeline omits the mode segment

## ADDED Requirements

### Requirement: Session mode lines identify model effort and title
A session mode line's left group SHALL contain, in order, the model and effort display names joined by ` · ` as one segment, the current mode segment, and the session title in `mode-line-buffer-id`. Model and effort SHALL come from the first select config options in categories `model` and `thought_level`, finding the current value in either flat or grouped options. The current value itself SHALL be the fallback when unlisted. Missing model or effort SHALL be omitted without a joining dot. These fields SHALL follow readiness and each config update or set-config answer. The title SHALL follow the same normalization and 40-character limit as the buffer name and fall back to the agent's registry name when unset. The generic buffer state, decorated buffer name and point position SHALL be replaced by these segments. Inactive styling and width fitting SHALL follow the existing mode-line rules.

#### Scenario: Ready configuration and title
- **GIVEN** a session titled `Fix it` with model `opus` named `Opus 5.5` and effort `high` named `High`
- **WHEN** its mode is `plan`
- **THEN** the left group reads `Opus 5.5 · High │ [plan] │ Fix it`

#### Scenario: Changed or grouped configuration
- **GIVEN** a ready session
- **WHEN** updated model or thought-level config has its current value in a group
- **THEN** the model line uses that value's display name

#### Scenario: Missing or unknown fields
- **GIVEN** a session with no title, model or effort
- **WHEN** its mode line is displayed
- **THEN** the title segment shows the agent name and unavailable fields are omitted

### Requirement: Session mode lines show lifecycle status
The right group SHALL show `● working` in `status-running` while a prompt awaits its answer, `● awaiting permission` in `status-waiting` while any permission request is pending, or `● exited` in `status-failed` after exit or failed startup, with exited taking precedence over awaiting permission, which takes precedence over working. A live session without a running prompt or pending permission SHALL omit status. Answering one of several requests SHALL keep awaiting-permission status until all are answered.

#### Scenario: Turn and permission lifecycle
- **GIVEN** a live idle session
- **WHEN** a prompt starts, two permissions arrive, one is answered, then the other is answered, then the prompt ends
- **THEN** the status progresses through working, awaiting permission, awaiting permission, working, then no status

#### Scenario: Session ends
- **GIVEN** a running session with a pending request or a starting session
- **WHEN** the agent exits or fails to start
- **THEN** its status is `● exited`

### Requirement: Session mode lines show context usage
The core SHALL retain the latest `usage_update`'s `used` and `size` and show `ctx N%` after status in `mode-line-dim`, where N is the integer floor of `used * 100 / size`, capped at 100. Before usage arrives, or when the latest size is zero, the context segment SHALL be omitted. Updates after exit SHALL be ignored. No cost or rate-limit segment SHALL be shown.

#### Scenario: Recorded usage
- **GIVEN** each recorded adapter session
- **WHEN** its recorded usage updates are replayed
- **THEN** each mode line's context percentage follows the latest used and size values

#### Scenario: Invalid or excessive usage
- **GIVEN** a session receiving usage
- **WHEN** size is zero, then used is greater than a positive size
- **THEN** context is first omitted and then shown as `ctx 100%`
