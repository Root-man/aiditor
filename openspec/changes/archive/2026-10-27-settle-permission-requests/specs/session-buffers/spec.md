## MODIFIED Requirements

### Requirement: Cancelling ends pending permission requests
When `C-c C-c` sends `session/cancel`, every pending permission request of the session SHALL be answered with the `cancelled` outcome. When the agent exits, pending requests are dropped unanswered. Either way each block SHALL become the line `Cancelled permission for <title>`. A permission request that arrives after the session ended, or after its session buffer was killed, SHALL be answered `cancelled` and SHALL NOT be shown.

#### Scenario: Cancelling with a pending request
- **WHEN** a request for `Edit a.txt` is pending and `C-c C-c` is typed
- **THEN** it is answered `cancelled`, `session/cancel` is sent and the block reads `Cancelled permission for Edit a.txt`

#### Scenario: A request after the session ended
- **WHEN** the agent has exited and a permission request from it arrives
- **THEN** it is answered `cancelled` and the session buffer is unchanged

#### Scenario: A request after the session buffer was killed
- **WHEN** the session buffer was killed and a permission request from its agent arrives before the agent exits
- **THEN** it is answered `cancelled`
