## ADDED Requirements

### Requirement: A closed connection ends the agent
When the agent closes its stdout after its session opened, the agent service SHALL terminate the agent as a kill op does: SIGTERM to its process group and SIGKILL after the grace. The core SHALL then get the agent's exit message after its stderr lines, as for any exit. An agent that closed its stdout because it already exited SHALL get the same exit message and nothing more.

#### Scenario: The agent stops speaking but keeps running
- **WHEN** the fake agent's session steps close its stdout and then sleep for 10 seconds
- **THEN** the core gets an exit message for that agent, ended by a signal, well before the sleep ends

#### Scenario: A turn in flight ends
- **WHEN** a prompt is sent to a fake agent whose turn closes its stdout and then sleeps
- **THEN** the core gets the agent's exit message, and the session is no longer busy
