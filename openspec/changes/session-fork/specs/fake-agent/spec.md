## ADDED Requirements

### Requirement: Forking a session
With `"fork": true` and a `history`, the fake SHALL advertise `sessionCapabilities.fork`; otherwise it SHALL NOT. On `session/fork` it SHALL report `fork=<session id>`, `at=<message id>` (empty without a fork point) and `dirs=` on stderr and answer with session `fake-fork` and the same config options as `session/new`. A later `session/load` SHALL replay the history up to and including the last update carrying that message id, or the whole history without one.

#### Scenario: A fork at a message cuts the replay
- **WHEN** the history is agent message chunks with ids `m1`, `m1` and `m2`, and the client forks at `m1` and loads `fake-fork`
- **THEN** the stderr has `fork=<source>` and `at=m1`, and the load replays the two `m1` chunks only
