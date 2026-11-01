# Spec Delta

## REMOVED Requirements

### Requirement: Session updates are kept
**Reason**: Nothing reads the kept updates, they hold every tool call's output a second time, and the requirement's claim that updates change no buffer's text was superseded by `session-buffers`.
**Migration**: None; the session buffer already shows what each update carries. The rule for unknown and ended sessions moves to its own requirement below.

## ADDED Requirements

### Requirement: Updates for unknown or ended sessions are dropped
A `session/update` notification for an unknown or ended session SHALL be dropped without changing any buffer's text.

#### Scenario: An update after the session ended
- **WHEN** an agent's session has ended and an agent message chunk update arrives for it
- **THEN** no buffer's text changes

#### Scenario: An update for an unknown session
- **WHEN** an agent message chunk update arrives for an agent id the core does not know
- **THEN** no buffer's text changes
